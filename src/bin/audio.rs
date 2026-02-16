#![no_std]
#![no_main]

use display_interface_i2c::I2CInterface;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::i2c::master::{Config, I2c};
use esp_hal::{clock::CpuClock, i2s::master as i2s, time::Rate, timer::timg::TimerGroup};
use oled_async::builder::Builder;
use rtt_target::rprintln;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    rprintln!("PANIC!!");
    loop {}
}

use pds::audio::{IS_PLAYING_SIGNAL, NEXT, PREVIOUS, audio_task, volume_handler_task};
use pds::button::button_task;
use pds::display::{OledDisplay, display_task};
use pds::encoder::encoder_reader_task;

// This creates a default app-descriptor required by the esp-idf bootloader.
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    rtt_target::rtt_init_log!();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 73744);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    rprintln!("Embassy initialized!");

    let radio_init = esp_radio::init().expect("Failed to initialize Wi-Fi/BLE controller");
    let (mut _wifi_controller, _interfaces) =
        esp_radio::wifi::new(&radio_init, peripherals.WIFI, Default::default())
            .expect("Failed to initialize Wi-Fi controller");

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

    let raw_disp = Builder::new(oled_async::displays::sh1106::Sh1106_128_64 {})
        // .with_rotation(DisplayRotation::Rotate180)
        .connect(di);
    let display: OledDisplay = raw_disp.into();

    spawner.spawn(display_task(display)).unwrap();
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

    spawner
        .spawn(button_task(
            peripherals.GPIO4.into(),
            "Encoder button",
            &IS_PLAYING_SIGNAL,
        ))
        .unwrap();

    spawner
        .spawn(button_task(peripherals.GPIO1.into(), "Next button", &NEXT))
        .unwrap();

    spawner
        .spawn(button_task(
            peripherals.GPIO7.into(),
            "Prev button",
            &PREVIOUS,
        ))
        .unwrap();

    spawner
        .spawn(encoder_reader_task(
            peripherals.GPIO3.into(),
            peripherals.GPIO2.into(),
        ))
        .unwrap();

    spawner.spawn(volume_handler_task()).unwrap();
    spawner.spawn(audio_task(i2s_tx, tx_buffer)).unwrap();

    // Escrever continuamente
    // let mut transfer = i2s_tx.write_dma_circular(tx_buffer).unwrap();
    // // -------- i2s
    // let mut audio_offset = 0;
    // let gain = 0.5;
    // loop {
    //     let avail = transfer.available().unwrap();
    //
    //     // ✅ Só empurrar quando houver BASTANTE espaço (não apenas > 0)
    //     if avail > 1024 {
    //         // ✅ Limitar tamanho do chunk (não usar todo o 'avail')
    //         let chunk_size = 512.min(avail).min(AUDIO_DATA.len() - audio_offset);
    //
    //         let audio_chunk = &AUDIO_DATA[audio_offset..audio_offset + chunk_size];
    //         // ✅ Aplicar ganho aos samples
    //         let mut amplified = [0u8; 512];
    //         for (i, sample_bytes) in audio_chunk.chunks_exact(2).enumerate() {
    //             // Converter bytes para i16
    //             let sample = i16::from_le_bytes([sample_bytes[0], sample_bytes[1]]);
    //
    //             // Multiplicar por ganho
    //             let amplified_sample = ((sample as f32) * gain) as i16;
    //
    //             // Converter de volta para bytes
    //             amplified[i * 2..i * 2 + 2].copy_from_slice(&amplified_sample.to_le_bytes());
    //         }
    //
    //         transfer.push(&amplified[..chunk_size]).unwrap();
    //
    //         audio_offset += chunk_size;
    //         if audio_offset >= AUDIO_DATA.len() {
    //             audio_offset = 0;
    //         }
    //     }
    // }
    loop {
        rprintln!("Hello world!");
        Timer::after(Duration::from_secs(5)).await;
    }
}
