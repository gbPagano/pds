use core::sync::atomic::Ordering;
use display_interface_i2c::I2CInterface;
use embassy_time::{Duration, Timer};
use embedded_graphics::{
    image::Image,
    mono_font::{MonoTextStyle, ascii::FONT_7X13_BOLD},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle},
    text::Text,
};
use esp_hal::{Async, i2c::master::I2c};
use oled_async::{displays::sh1106, mode::GraphicsMode};
use tinybmp::Bmp;

use crate::assets::{
    NEXT_BYTES, PAUSE_BYTES, PLAY_BYTES, PREV_BYTES, SOUND_ICON_BYTES, SOUND_WAVE_BYTES,
};
use crate::audio::{CURRENT_MUSIC_INDEX, CURRENT_PERCENTAGE, IS_PLAYING, VOLUME};
use crate::music::Musics;

/// Type alias for the SH1106 OLED display using I2C and Async mode.
pub type OledDisplay = GraphicsMode<sh1106::Sh1106_128_64, I2CInterface<I2c<'static, Async>>>;

/// Main task for UI rendering.
#[embassy_executor::task]
pub async fn display_task(mut display: OledDisplay) {
    display.init().await.unwrap();
    display.flush().await.unwrap();

    let style = MonoTextStyle::new(&FONT_7X13_BOLD, BinaryColor::On);
    // Load animated sound wave GIF from static assets
    let wave_gif = tinygif::Gif::<BinaryColor>::from_slice(SOUND_WAVE_BYTES).unwrap();
    let mut wave_iter = wave_gif.frames();
    let mut current_frame = wave_iter.next().unwrap();
    loop {
        display.clear();

        // --- 1. Animation Logic ---
        // Increment GIF frame only if audio is playing
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

        // --- 2. Track Title ---
        let curr_music = Musics::from_index(&CURRENT_MUSIC_INDEX.load(Ordering::Relaxed));
        Text::new(curr_music.title(), curr_music.title_pos(), style)
            .draw(&mut display)
            .unwrap();

        // --- 3. Sound Visualizer ---
        // Renders the current git frame for a moving effect
        let (x, y) = (23, 22);
        for offset in [42, 21, 0] {
            current_frame
                .draw(&mut display.translated(Point::new(x + offset, y)))
                .unwrap();
        }

        // --- 4. Control Icons (BMP) ---
        // Next & Previous
        Image::new(
            &Bmp::from_slice(NEXT_BYTES).unwrap(),
            Point::new(x + 68, y + 4),
        )
        .draw(&mut display)
        .unwrap();
        Image::new(
            &Bmp::from_slice(PREV_BYTES).unwrap(),
            Point::new(x - 18, y + 4),
        )
        .draw(&mut display)
        .unwrap();

        // Play/Pause toggle icon
        Image::new(
            &Bmp::from_slice(get_play_pause_icon()).unwrap(),
            Point::new(6, 52),
        )
        .draw(&mut display)
        .unwrap();

        // --- 5. Gauges ---
        // Playback progress (Horizontal)
        draw_progress_bar(
            &mut display,
            CURRENT_PERCENTAGE.load(Ordering::Relaxed),
            Point::new(20, 52),
            Size::new(80, 10),
            Orientation::Horizontal,
        )
        .unwrap();

        // Volume level (Vertical)
        draw_progress_bar(
            &mut display,
            VOLUME.load(Ordering::Relaxed),
            Point::new(115, 3),
            Size::new(10, 45),
            Orientation::Vertical,
        )
        .unwrap();

        // Volume icon
        Image::new(
            &Bmp::from_slice(SOUND_ICON_BYTES).unwrap(),
            Point::new(115, 52),
        )
        .draw(&mut display)
        .unwrap();

        // Send buffer to the physical display
        display.flush().await.unwrap();

        // Frame rate control: Fast refresh for animation, slow refresh when idle
        if IS_PLAYING.load(Ordering::Relaxed) {
            let delay = (current_frame.delay_centis as u64) * 3;
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

/// Draws a stylized progress bar.
/// Supports both Horizontal (fill from left) and Vertical (fill from bottom) orientations.
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
            let max_w = size.width - (margin * 2);
            let current_w = (max_w * safe_progress as u32) / 100;
            (
                Size::new(current_w, size.height - (margin * 2)),
                position + Point::new(margin as i32, margin as i32),
            )
        }
        Orientation::Vertical => {
            let max_h = size.height - (margin * 2);
            let current_h = (max_h * safe_progress as u32) / 100;
            // Fill from bottom to top
            let y_offset = (size.height - margin) - current_h;
            (
                Size::new(size.width - (margin * 2), current_h),
                position + Point::new(margin as i32, y_offset as i32),
            )
        }
    };

    if fill_size.width > 0 && fill_size.height > 0 {
        Rectangle::new(fill_position, fill_size)
            .into_styled(fill_style)
            .draw(target)?;
    }

    Ok(())
}
