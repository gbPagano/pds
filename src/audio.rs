use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer};
use esp_hal::{Blocking, i2s::master::I2sTx};

use crate::button::ButtonSignal;
use crate::encoder::{ENCODER_CHANNEL, EncoderDirection};
use crate::music::Musics;

pub static VOLUME: AtomicU8 = AtomicU8::new(50); // initial volume to 50%
pub static CURRENT_PERCENTAGE: AtomicU8 = AtomicU8::new(0);
pub static IS_PLAYING_SIGNAL: ButtonSignal = Signal::new();
pub static IS_PLAYING: AtomicBool = AtomicBool::new(false);
pub static NEXT: ButtonSignal = Signal::new();
pub static PREVIOUS: ButtonSignal = Signal::new();
pub static CURRENT_MUSIC_INDEX: AtomicU8 = AtomicU8::new(0);

#[embassy_executor::task]
pub async fn volume_handler_task() {
    loop {
        let direction = ENCODER_CHANNEL.receive().await;

        match direction {
            EncoderDirection::Clockwise => {
                // Increase max to 100
                VOLUME
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                        if v < 100 { Some(v + 5) } else { Some(100) }
                    })
                    .ok();
            }

            EncoderDirection::CounterClockwise => {
                // decrease min to 0
                VOLUME
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                        if v > 0 { Some(v - 5) } else { Some(0) }
                    })
                    .ok();
            }
        }

        let volume_level = VOLUME.load(Ordering::Relaxed);
        log::info!("Volume changed: {volume_level}");
    }
}

#[embassy_executor::task]
pub async fn audio_task(
    mut i2s_tx: I2sTx<'static, Blocking>,
    tx_buffer: &'static mut [u8; 4 * 4092],
) {
    // Inicializa o transfer DMA circular
    let mut transfer = i2s_tx.write_dma_circular(tx_buffer).unwrap();

    let mut current_music = Musics::from_index(&CURRENT_MUSIC_INDEX.load(Ordering::Relaxed));
    let mut audio_data = current_music.bytes();
    let mut total_len = audio_data.len();

    let mut audio_offset = 0;
    let mut is_playing = IS_PLAYING.load(Ordering::Relaxed);

    // Controle de tempo para o Log
    let mut last_log_time = Instant::now();
    loop {
        // --- Check: Play/Pause ---
        if IS_PLAYING_SIGNAL.try_take().is_some() {
            is_playing = !is_playing;
            IS_PLAYING.store(is_playing, Ordering::Relaxed);
            log::info!("Play/pause");
        }

        // --- Check: Next Music ---
        if NEXT.try_take().is_some() {
            current_music = current_music.next();
            CURRENT_MUSIC_INDEX.store(current_music.to_index(), Ordering::Relaxed);
            audio_data = current_music.bytes();
            total_len = audio_data.len();
            audio_offset = 0;
            CURRENT_PERCENTAGE.store(0, Ordering::Relaxed);

            // Inicia a reprodução automaticamente
            is_playing = true;
            IS_PLAYING.store(true, Ordering::Relaxed);

            log::info!("Next music: {}", current_music.title());
        }

        // --- Check: Previous Music ---
        if PREVIOUS.try_take().is_some() {
            // Reset treshold 10%
            if (audio_offset * 100) / total_len > 10 {
                audio_offset = 0;
                CURRENT_PERCENTAGE.store(0, Ordering::Relaxed);
                log::info!("Restarting current music: {}", current_music.title());
            } else {
                current_music = current_music.prev();
                CURRENT_MUSIC_INDEX.store(current_music.to_index(), Ordering::Relaxed);
                audio_data = current_music.bytes();
                total_len = audio_data.len();
                audio_offset = 0;
                CURRENT_PERCENTAGE.store(0, Ordering::Relaxed);
                log::info!("Previous music: {}", current_music.title());
            }
            // Inicia a reprodução automaticamente
            is_playing = true;
            IS_PLAYING.store(true, Ordering::Relaxed);
        }

        // ====================================================================
        // 2. PROCESSAMENTO DE ÁUDIO
        // ====================================================================

        let avail = transfer.available().unwrap();

        if !is_playing {
            let silence = [0u8; 512]; // Buffer temporário de silêncio
            let chunk = avail.min(512);

            transfer.push(&silence[..chunk]).unwrap();

            Timer::after(Duration::from_millis(10)).await;
            continue;
        }

        if avail > 1024 {
            let chunk_size = 512.min(avail).min(audio_data.len() - audio_offset);
            let audio_chunk = &audio_data[audio_offset..audio_offset + chunk_size];

            // Buffer temporário para processar o ganho
            let mut amplified = [0u8; 512];
            let volume_level = VOLUME.load(Ordering::Relaxed);
            let gain = (volume_level as f32) / 100.0;

            for (i, sample_bytes) in audio_chunk.chunks_exact(2).enumerate() {
                let sample = i16::from_le_bytes([sample_bytes[0], sample_bytes[1]]);

                // Aplica o ganho dinâmico lido do encoder
                let amplified_sample = ((sample as f32) * gain) as i16;

                amplified[i * 2..i * 2 + 2].copy_from_slice(&amplified_sample.to_le_bytes());
            }

            // Envia para o DMA
            transfer.push(&amplified[..chunk_size]).unwrap();

            if last_log_time.elapsed() > Duration::from_secs(1) {
                let percent = (audio_offset * 100) / total_len;
                CURRENT_PERCENTAGE.store(percent as u8, Ordering::Relaxed);
                log::info!("Playing: {percent}%");
                last_log_time = Instant::now();
            }

            audio_offset += chunk_size;
            if audio_offset >= audio_data.len() {
                audio_offset = 0;
                is_playing = false;
                IS_PLAYING.store(is_playing, Ordering::Relaxed);
                CURRENT_PERCENTAGE.store(0, Ordering::Relaxed);
                log::info!("Music '{}' ended!", current_music.title());
            }
        }
        Timer::after(Duration::from_millis(5)).await;
    }
}
