use core::sync::atomic::Ordering;
use display_interface_i2c::I2CInterface;
use embassy_time::{Duration, Timer};
use embedded_graphics::image::Image;
use embedded_graphics::mono_font::ascii::FONT_7X13_BOLD;
use embedded_graphics::{
    mono_font::MonoTextStyle,
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle},
    text::Text,
};
use esp_hal::Async;
use esp_hal::i2c::master::I2c;
use oled_async::displays::sh1106;
use oled_async::mode::GraphicsMode;
use tinybmp::Bmp;

use crate::assets::{
    NEXT_BYTES, PAUSE_BYTES, PLAY_BYTES, PREV_BYTES, SOUND_ICON_BYTES, SOUND_WAVE_BYTES,
};
use crate::music::{CURRENT_PERCENTAGE, IS_PLAYING, VOLUME};

pub type OledDisplay = GraphicsMode<sh1106::Sh1106_128_64, I2CInterface<I2c<'static, Async>>>;

#[embassy_executor::task]
pub async fn display_task(mut display: OledDisplay) {
    display.init().await.unwrap();

    let wave_gif = tinygif::Gif::<BinaryColor>::from_slice(SOUND_WAVE_BYTES).unwrap();

    let style = MonoTextStyle::new(&FONT_7X13_BOLD, BinaryColor::On);
    display.clear();
    display.flush().await.unwrap();

    let mut wave_iter = wave_gif.frames();
    let mut current_frame = wave_iter.next().unwrap();
    loop {
        if IS_PLAYING.load(Ordering::Relaxed) {
            match wave_iter.next() {
                Some(frame) => {
                    current_frame = frame;
                }
                None => {
                    wave_iter = wave_gif.frames();
                    current_frame = wave_iter.next().unwrap();
                }
            }
        }

        Text::new("Tetris", Point::new(35, 15), style)
            .draw(&mut display)
            .unwrap();

        // Current gif frame
        let (x, y) = (23, 22);
        current_frame
            .draw(&mut display.translated(Point::new(x + 42, y)))
            .unwrap();
        current_frame
            .draw(&mut display.translated(Point::new(x + 21, y)))
            .unwrap();
        current_frame
            .draw(&mut display.translated(Point::new(x, y)))
            .unwrap();

        // next and prev icons
        let bmp: Bmp<BinaryColor> = Bmp::from_slice(NEXT_BYTES).unwrap();
        let image = Image::new(&bmp, Point::new(x + 68, y + 4));
        image.draw(&mut display).unwrap();
        let bmp: Bmp<BinaryColor> = Bmp::from_slice(PREV_BYTES).unwrap();
        let image = Image::new(&bmp, Point::new(x - 18, y + 4));
        image.draw(&mut display).unwrap();

        // play pause icon
        let bmp: Bmp<BinaryColor> = Bmp::from_slice(get_play_pause_icon()).unwrap();
        let image = Image::new(&bmp, Point::new(6, 52));
        image.draw(&mut display).unwrap();

        // progress bar
        draw_progress_bar(
            &mut display,
            CURRENT_PERCENTAGE.load(Ordering::Relaxed),
            Point::new(20, 52),
            Size::new(80, 10),
            Orientation::Horizontal,
        )
        .unwrap();

        // volume bar
        draw_progress_bar(
            &mut display,
            VOLUME.load(Ordering::Relaxed),
            Point::new(115, 3),
            Size::new(10, 45),
            Orientation::Vertical,
        )
        .unwrap();

        // sound icon
        let bmp: Bmp<BinaryColor> = Bmp::from_slice(SOUND_ICON_BYTES).unwrap();
        let image = Image::new(&bmp, Point::new(115, 52));
        image.draw(&mut display).unwrap();

        display.flush().await.unwrap();

        if IS_PLAYING.load(Ordering::Relaxed) {
            let delay = (current_frame.delay_centis as u64) * 3;
            log::debug!("delay gif: {delay}");
            Timer::after(Duration::from_millis(delay.max(10))).await;
        } else {
            Timer::after(Duration::from_millis(100)).await;
        }
    }
}

fn get_play_pause_icon() -> &'static [u8] {
    if IS_PLAYING.load(Ordering::Relaxed) {
        PAUSE_BYTES
    } else {
        PLAY_BYTES
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

/// Generic progress bar drawing function
fn draw_progress_bar<D>(
    target: &mut D,
    progress: u8,
    position: Point,
    size: Size,
    orientation: Orientation,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = BinaryColor>,
{
    let border_style = PrimitiveStyleBuilder::new()
        .stroke_color(BinaryColor::On)
        .stroke_width(1)
        .build();

    let fill_style = PrimitiveStyleBuilder::new()
        .fill_color(BinaryColor::On)
        .build();

    Rectangle::new(position, size)
        .into_styled(border_style)
        .draw(target)?;

    let safe_progress = progress.min(100);
    let margin = 2;

    let (fill_size, fill_position) = match orientation {
        Orientation::Horizontal => {
            let max_width = size.width - (margin * 2);
            let current_width = (max_width * safe_progress as u32) / 100;
            let fill_size = Size::new(current_width, size.height - (margin * 2));
            let fill_position = position + Point::new(margin as i32, margin as i32);
            (fill_size, fill_position)
        }
        Orientation::Vertical => {
            let max_height = size.height - (margin * 2);
            let current_height = (max_height * safe_progress as u32) / 100;
            let fill_size = Size::new(size.width - (margin * 2), current_height);
            let y_offset = (size.height - margin) - current_height;
            let fill_position = position + Point::new(margin as i32, y_offset as i32);
            (fill_size, fill_position)
        }
    };

    if fill_size.width > 0 && fill_size.height > 0 {
        Rectangle::new(fill_position, fill_size)
            .into_styled(fill_style)
            .draw(target)?;
    }

    Ok(())
}
