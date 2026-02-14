#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::PIO0;
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::pio_programs::i2s::{PioI2sOut, PioI2sOutProgram};
use embassy_time::Timer;
use log::*;
use panic_usb_boot as _;

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

static AUDIO_DATA: &[u8] = include_bytes!("../tetris.raw");

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("🎵 Iniciando reprodução de áudio I2S com módulo Embassy");

    let data_pin = p.PIN_3; // DIN
    let clk_pin = p.PIN_26; // BCK
    let ws_pin = p.PIN_27; // LRCK

    let Pio {
        mut common, sm0, ..
    } = Pio::new(p.PIO0, Irqs);

    let prg = PioI2sOutProgram::new(&mut common);

    let mut i2s = PioI2sOut::new(
        &mut common,
        sm0,
        p.DMA_CH0,
        data_pin,
        clk_pin,
        ws_pin,
        11025, // 11025 Hz
        16,    // 16 bits
        &prg,
    );

    const CHUNK_SIZE: usize = 1024;
    let mut buffer = [0u32; CHUNK_SIZE];

    let mut idx = 0;

    loop {
        for i in 0..CHUNK_SIZE {
            if idx + 1 >= AUDIO_DATA.len() {
                idx = 0;
            }

            let b1 = AUDIO_DATA[idx];
            let b2 = AUDIO_DATA[idx + 1];

            // u32: [b1, b2, b1, b2]
            buffer[i] = u32::from_le_bytes([b1, b2, b1, b2]);

            idx += 2;
        }

        i2s.write(&buffer).await;
    }
}
