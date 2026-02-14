#![no_std]
#![no_main]

use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Duration, Timer};
use esp_hal::{
    clock::CpuClock,
    gpio::{AnyPin, Input, InputConfig, Io, Level, Pull},
    pcnt::{Pcnt, channel::EdgeMode, unit::Unit},
    timer::timg::TimerGroup,
};
use log::*;

use pds::button::*;
use pds::encoder::*;
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
esp_bootloader_esp_idf::esp_app_desc!();

// Sinais para comunicação entre tasks
static BUTTON_SIGNAL: Signal<CriticalSectionRawMutex, bool> = Signal::new();
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    rtt_target::rtt_init_log!();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 73744);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    log::info!("Embassy initialized!");

    spawner
        .spawn(encoder_reader_task(
            peripherals.GPIO3.into(),
            peripherals.GPIO2.into(),
        ))
        .unwrap();
    spawner
        .spawn(button_task(
            peripherals.GPIO4.into(),
            "Encoder button",
            &BUTTON_SIGNAL,
        ))
        .unwrap();

    loop {
        log::info!("Hello world!");
        Timer::after(Duration::from_secs(5)).await;
    }
}
