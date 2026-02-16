#![no_std]
#![no_main]

use display_interface_i2c::I2CInterface;
use embassy_executor::Spawner;
use esp_hal::{
    clock::CpuClock,
    i2c::master::{Config, I2c},
    i2s::master as i2s,
    time::Rate,
    timer::timg::TimerGroup,
};
use oled_async::builder::Builder;
use panic_rtt_target as _; // this defines panic handler

use pds::audio::{IS_PLAYING_SIGNAL, NEXT, PREVIOUS, audio_task, volume_handler_task};
use pds::button::button_task;
use pds::display::{OledDisplay, display_task};
use pds::encoder::encoder_reader_task;

// This creates a default app-descriptor required by the esp-idf bootloader.
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    rtt_target::rtt_init_log!();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    // --------- i2c
    let i2c = I2c::new(peripherals.I2C0, Config::default())
        .unwrap()
        .with_sda(peripherals.GPIO5)
        .with_scl(peripherals.GPIO6);

    let i2c_async = i2c.into_async();

    let di = I2CInterface::new(
        i2c_async, // I2C
        0x3C,      // I2C Address
        0x40,      // Databyte
    );

    let raw_disp = Builder::new(oled_async::displays::sh1106::Sh1106_128_64 {}).connect(di);
    let display: OledDisplay = raw_disp.into();

    // -------- i2s
    let dma_channel = peripherals.DMA_CH0;
    let (_, _, tx_buffer, tx_descriptors) = esp_hal::dma_buffers!(0, 4 * 4092);

    let i2s = i2s::I2s::new(
        peripherals.I2S0,
        dma_channel,
        i2s::Config::new_tdm_philips()
            .with_sample_rate(Rate::from_hz(11025))
            .with_data_format(i2s::DataFormat::Data16Channel16)
            .with_channels(i2s::Channels::MONO),
    )
    .unwrap();

    let i2s_tx = i2s
        .i2s_tx
        .with_bclk(peripherals.GPIO8)
        .with_ws(peripherals.GPIO9)
        .with_dout(peripherals.GPIO10)
        .build(tx_descriptors);

    // spawn tasks
    spawner
        .spawn(button_task(
            peripherals.GPIO4.into(),
            "Encoder",
            &IS_PLAYING_SIGNAL,
        ))
        .unwrap();
    spawner
        .spawn(button_task(peripherals.GPIO1.into(), "Prev", &PREVIOUS))
        .unwrap();
    spawner
        .spawn(button_task(peripherals.GPIO7.into(), "Next", &NEXT))
        .unwrap();
    spawner
        .spawn(encoder_reader_task(
            peripherals.GPIO3.into(),
            peripherals.GPIO2.into(),
        ))
        .unwrap();
    spawner.spawn(volume_handler_task()).unwrap();
    spawner.spawn(display_task(display)).unwrap();
    spawner.spawn(audio_task(i2s_tx, tx_buffer)).unwrap();
}
