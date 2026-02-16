use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Duration, Timer};
use esp_hal::gpio::{AnyPin, Input, InputConfig, Pull};

/// A thread-safe signal to notify tasks of button events.
pub type ButtonSignal = Signal<CriticalSectionRawMutex, bool>;

/// Monitors a GPIO pin for button presses with 20ms debouncing.
///
/// # Parameters
/// - `pin_gpio`: GPIO pin to monitor
/// - `id`: Label used for logging
/// - `signal`: The signal to trigger on a valid press.
#[embassy_executor::task(pool_size = 3)]
pub async fn button_task(
    pin_gpio: AnyPin<'static>,
    id: &'static str,
    signal: &'static ButtonSignal,
) {
    let config = InputConfig::default().with_pull(Pull::Up);
    let mut button = Input::new(pin_gpio, config);

    loop {
        button.wait_for_falling_edge().await;

        Timer::after(Duration::from_millis(20)).await; // Debounce

        if button.is_low() {
            log::debug!("{id} button pressed!");
            signal.signal(true);
        }
    }
}
