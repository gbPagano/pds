use core::sync::atomic::{AtomicU8, Ordering};
use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer};
use esp_hal::Blocking;
use esp_hal::dma::DmaTransferTxCircular;
use esp_hal::gpio::{AnyPin, Input, InputConfig, Pull};
use esp_hal::i2s::master::I2sTx;

use crate::button::ButtonSignal;
use crate::encoder::{ENCODER_CHANNEL, EncoderDirection};

pub static VOLUME: AtomicU8 = AtomicU8::new(50); // initial volume to 50%
pub static IS_PLAYING: ButtonSignal = Signal::new();
pub static NEXT: ButtonSignal = Signal::new();
pub static PREVIOUS: ButtonSignal = Signal::new();

const AUDIO_DATA: &[u8] = include_bytes!("../tetris.raw");

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

    let mut audio_offset = 0;
    let mut is_paused = false; // Estado local de pausa
    let total_len = AUDIO_DATA.len();

    // Controle de tempo para o Log
    let mut last_log_time = Instant::now();
    loop {
        // ====================================================================
        // 1. VERIFICAÇÃO DE SINAIS (Controle)
        // ====================================================================

        // --- Check: Play/Pause ---
        if IS_PLAYING.try_take().is_some() {
            is_paused = !is_paused;
            log::info!("Play/pause");
        }

        // ====================================================================
        // 2. PROCESSAMENTO DE ÁUDIO
        // ====================================================================

        let avail = transfer.available().unwrap();

        if is_paused {
            let silence = [0u8; 512]; // Buffer temporário de silêncio
            let chunk = avail.min(512);

            transfer.push(&silence[..chunk]).unwrap();

            Timer::after(Duration::from_millis(10)).await;
            continue;
        }

        if avail > 1024 {
            let chunk_size = 512.min(avail).min(AUDIO_DATA.len() - audio_offset);

            let audio_chunk = &AUDIO_DATA[audio_offset..audio_offset + chunk_size];

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
                log::info!("Playing: {percent}%");
                last_log_time = Instant::now();
            }

            audio_offset += chunk_size;
            if audio_offset >= AUDIO_DATA.len() {
                audio_offset = 0;
                is_paused = true;
                log::info!("Music ended!");
            }
        }
        Timer::after(Duration::from_millis(5)).await;
    }
}
