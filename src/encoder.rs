use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::{Duration, Timer};
use esp_hal::gpio::{AnyPin, Input, InputConfig, Pull};

/// Channel for encoder rotation events (buffer size: 10).
pub static ENCODER_CHANNEL: Channel<CriticalSectionRawMutex, EncoderDirection, 10> = Channel::new();

/// Represents the direction of encoder rotation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EncoderDirection {
    Clockwise,
    CounterClockwise,
}

/// Reads a rotary encoder and sends rotation events to `ENCODER_CHANNEL`.
///
/// Detects rotation direction by reading pin B state when pin A rises.
/// Uses 50μs + 20ms debouncing to filter noise.
#[embassy_executor::task]
pub async fn encoder_reader_task(pin_a: AnyPin<'static>, pin_b: AnyPin<'static>) {
    let config = InputConfig::default().with_pull(Pull::Up);
    let mut tra = Input::new(pin_a, config);
    let trb = Input::new(pin_b, config);

    loop {
        tra.wait_for_rising_edge().await;

        Timer::after(Duration::from_micros(50)).await; // Debounce

        // Pin A must remain high after the debounce delay
        // If it is not, the edge was likely caused by noise
        if tra.is_high() {
            let direction = if trb.is_low() {
                EncoderDirection::Clockwise
            } else {
                EncoderDirection::CounterClockwise
            };
            ENCODER_CHANNEL.send(direction).await;

            log::debug!("Encoder: {direction:?}");
        }

        tra.wait_for_falling_edge().await;

        Timer::after(Duration::from_millis(20)).await; // Debounce 
    }
}
