use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer};
use esp_hal::{Blocking, i2s::master::I2sTx};

use crate::button::ButtonSignal;
use crate::encoder::{ENCODER_CHANNEL, EncoderDirection};
use crate::music::Musics;

/// Shared system volume (0-100%).
pub static VOLUME: AtomicU8 = AtomicU8::new(50);
/// Current playback progress percentage.
pub static CURRENT_PERCENTAGE: AtomicU8 = AtomicU8::new(0);
/// Signal to toggle Play/Pause state.
pub static IS_PLAYING_SIGNAL: ButtonSignal = Signal::new();
/// Atomic flag for current playback status.
pub static IS_PLAYING: AtomicBool = AtomicBool::new(false);
/// Signal to trigger next track.
pub static NEXT: ButtonSignal = Signal::new();
/// Signal to trigger previous track or restart current.
pub static PREVIOUS: ButtonSignal = Signal::new();
/// Index of the currently loaded track.
pub static CURRENT_MUSIC_INDEX: AtomicU8 = AtomicU8::new(0);

/// DMA buffer size configuration.
/// 4092 bytes is the hardware limit for a single ESP32 DMA descriptor.
/// We use a multiplier of 4 to create a circular buffer of ~16KB.
pub const DMA_BUFFER_SIZE: usize = 4 * 4092;

/// Handles volume adjustments based on rotary encoder input.
/// Listens to ENCODER_CHANNEL and updates the global VOLUME atomic.
#[embassy_executor::task]
pub async fn volume_handler_task() {
    loop {
        let direction = ENCODER_CHANNEL.receive().await;

        match direction {
            EncoderDirection::Clockwise => {
                VOLUME
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                        Some((v + 5).min(100)) // Cap at 100%
                    })
                    .ok();
            }

            EncoderDirection::CounterClockwise => {
                // decrease min to 0
                VOLUME
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                        Some(v.saturating_sub(5)) // Floor at 0%
                    })
                    .ok();
            }
        }

        log::info!("Volume changed: {}", VOLUME.load(Ordering::Relaxed));
    }
}

/// Core audio engine task.
/// Manages I2S DMA transfers, track switching, and real-time gain scaling.
#[embassy_executor::task]
pub async fn audio_task(
    mut i2s_tx: I2sTx<'static, Blocking>,
    tx_buffer: &'static mut [u8; DMA_BUFFER_SIZE],
) {
    // Initialize circular DMA transfer for continuous playback
    let mut transfer = i2s_tx.write_dma_circular(tx_buffer).unwrap();

    let mut current_music = Musics::from_index(&CURRENT_MUSIC_INDEX.load(Ordering::Relaxed));
    let mut audio_data = current_music.bytes();
    let mut total_len = audio_data.len();

    let mut audio_offset = 0;
    let mut is_playing = IS_PLAYING.load(Ordering::Relaxed);
    let mut last_log_time = Instant::now();

    loop {
        // 1. Handle Control Signals (Play/Pause/Next/Prev)
        if IS_PLAYING_SIGNAL.try_take().is_some() {
            is_playing = !is_playing;
            log::info!("Play/pause");
        }
        if NEXT.try_take().is_some() {
            let new_music = current_music.next();
            load_track(
                &mut current_music,
                &mut audio_data,
                &mut total_len,
                &mut audio_offset,
                new_music,
            );
            is_playing = true;
            log::info!("Next music: {}", current_music.title());
        }

        if PREVIOUS.try_take().is_some() {
            // Restart if >10% played, otherwise go to previous track
            if (audio_offset * 100) / total_len > 10 {
                audio_offset = 0;
                CURRENT_PERCENTAGE.store(0, Ordering::Relaxed);
                log::info!("Restarting current music: {}", current_music.title());
            } else {
                let new_music = current_music.prev();
                load_track(
                    &mut current_music,
                    &mut audio_data,
                    &mut total_len,
                    &mut audio_offset,
                    new_music,
                );
                log::info!("Previous music: {}", current_music.title());
            }
            is_playing = true;
        }

        IS_PLAYING.store(is_playing, Ordering::Relaxed);

        // 2. Audio Processing & DMA Feed
        let avail = transfer.available().unwrap();

        if !is_playing {
            // Feed silence to prevent audio artifacts while paused
            let silence = [0u8; 512];
            let chunk = avail.min(512);
            transfer.push(&silence[..chunk]).unwrap();
            Timer::after(Duration::from_millis(10)).await;
            continue;
        }

        if avail > 1024 {
            let chunk_size = 512.min(avail).min(audio_data.len() - audio_offset);
            let audio_chunk = &audio_data[audio_offset..audio_offset + chunk_size];

            let mut amplified = [0u8; 512];
            let volume_level = VOLUME.load(Ordering::Relaxed);
            let gain = (volume_level as f32) / 100.0;

            // Apply software volume scaling (Gain) to 16-bit PCM samples
            for (i, sample_bytes) in audio_chunk.chunks_exact(2).enumerate() {
                let sample = i16::from_le_bytes([sample_bytes[0], sample_bytes[1]]);
                let amplified_sample = ((sample as f32) * gain) as i16;
                amplified[i * 2..i * 2 + 2].copy_from_slice(&amplified_sample.to_le_bytes());
            }
            // Send to DMA
            transfer.push(&amplified[..chunk_size]).unwrap();
            audio_offset += chunk_size;

            // Track Progress Logging
            if last_log_time.elapsed() > Duration::from_secs(1) {
                let percent = (audio_offset * 100) / total_len;
                CURRENT_PERCENTAGE.store(percent as u8, Ordering::Relaxed);
                log::info!("Playing: {percent}%");
                last_log_time = Instant::now();
            }

            // Stop at EOF
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

/// Helper to update track state (Internal logic)
fn load_track(
    music: &mut Musics,
    data: &mut &[u8],
    total: &mut usize,
    offset: &mut usize,
    new_music: Musics,
) {
    *music = new_music;
    CURRENT_MUSIC_INDEX.store(music.to_index(), Ordering::Relaxed);
    *data = music.bytes();
    *total = data.len();
    *offset = 0;
    CURRENT_PERCENTAGE.store(0, Ordering::Relaxed);
}
