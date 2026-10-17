use std::io::Write;

use ratatui::{
    Frame, Terminal,
    backend::Backend,
    layout::Rect,
    style::{Color, Style},
};

use crate::app::{App, state::View};
use crate::errors::{AppError, AppResult};
use crate::terminal::ImageProtocol;

/// Assumed text-cell size in device pixels, used to size Sixel/kitty images so
/// they roughly fill the reserved rectangle.
const CELL_W: u32 = 10;
const CELL_H: u32 = 20;

/// Decoded cover art in RGB, small enough to keep in memory.
#[derive(Clone, Debug)]
pub struct CoverImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<[u8; 3]>,
}

impl CoverImage {
    pub fn average_color(&self) -> (u8, u8, u8) {
        if self.pixels.is_empty() {
            return (128, 128, 128);
        }
        let mut totals = [0u64; 3];
        for pixel in &self.pixels {
            totals[0] += u64::from(pixel[0]);
            totals[1] += u64::from(pixel[1]);
            totals[2] += u64::from(pixel[2]);
        }
        let count = self.pixels.len() as u64;
        (
            (totals[0] / count) as u8,
            (totals[1] / count) as u8,
            (totals[2] / count) as u8,
        )
    }

    fn pixel(&self, x: u32, y: u32) -> [u8; 3] {
        let x = x.min(self.width.saturating_sub(1));
        let y = y.min(self.height.saturating_sub(1));
        match self.pixels.get((y * self.width + x) as usize) {
            Some(pixel) => *pixel,
            None => [0, 0, 0],
        }
    }
}

/// Downloads and decodes a thumbnail, downscaled to a bounded size.
pub async fn fetch(url: &str) -> AppResult<CoverImage> {
    let client = reqwest::Client::builder()
        .http1_only()
        .build()
        .map_err(|error| AppError::Provider(format!("cover client: {error}")))?;
    let bytes = client
        .get(url)
        .send()
        .await
        .map_err(|error| AppError::Provider(format!("cover request: {error}")))?
        .error_for_status()
        .map_err(|error| AppError::Provider(format!("cover status: {error}")))?
        .bytes()
        .await
        .map_err(|error| AppError::Provider(format!("cover body: {error}")))?;
    decode(&bytes)
}

pub fn decode(bytes: &[u8]) -> AppResult<CoverImage> {
    let image = image::load_from_memory(bytes)
        .map_err(|error| AppError::Provider(format!("cover decode: {error}")))?;
    let thumb = image.thumbnail(160, 160).into_rgb8();
    let width = thumb.width();
    let height = thumb.height();
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for pixel in thumb.pixels() {
        pixels.push([pixel[0], pixel[1], pixel[2]]);
    }
    Ok(CoverImage {
        width,
        height,
        pixels,
    })
}

/// Paints the cover with upper/lower half blocks, which work in any truecolor
/// terminal. Geometry comes from the shared layout module.
pub fn draw(frame: &mut Frame<'_>, area: Rect, cover: &CoverImage) {
    if area.width == 0 || area.height == 0 || cover.width == 0 || cover.height == 0 {
        return;
    }
    let cells_w = area.width as f32;
    let cells_h = area.height as f32;
    let pixel_rows = cells_h * 2.0;
    let image_ratio = cover.width as f32 / cover.height as f32;
    let area_ratio = cells_w / pixel_rows;

    let (draw_w, draw_h) = if area_ratio > image_ratio {
        (pixel_rows * image_ratio, pixel_rows)
    } else {
        (cells_w, cells_w / image_ratio)
    };
    let offset_x = (cells_w - draw_w) / 2.0;
    let offset_y = (pixel_rows - draw_h) / 2.0;

    for row in 0..area.height {
        for column in 0..area.width {
            let cell_x = column as f32 + 0.5;
            let top_y = row as f32 * 2.0 + 0.5;
            let bottom_y = top_y + 1.0;
            let (fx, fy_top, fy_bottom) = (
                (cell_x - offset_x) / draw_w,
                (top_y - offset_y) / draw_h,
                (bottom_y - offset_y) / draw_h,
            );
            if !(0.0..1.0).contains(&fx) || !(0.0..1.0).contains(&fy_top) {
                continue;
            }
            let top = cover.pixel(
                (fx * cover.width as f32) as u32,
                (fy_top * cover.height as f32) as u32,
            );
            let bottom_ratio = fy_bottom.clamp(0.0, 0.9999);
            let bottom = cover.pixel(
                (fx * cover.width as f32) as u32,
                (bottom_ratio * cover.height as f32) as u32,
            );
            let cell = frame
                .buffer_mut()
                .cell_mut((area.x + column, area.y + row));
            if let Some(cell) = cell {
                cell.set_symbol("\u{2580}");
                cell.set_style(Style::default().fg(rgb(top)).bg(rgb(bottom)));
            }
        }
    }
}

fn rgb(pixel: [u8; 3]) -> Color {
    Color::Rgb(pixel[0], pixel[1], pixel[2])
}

/// Samples the cover with nearest-neighbour at the given pixel size.
fn sample_rgb(cover: &CoverImage, px_w: u32, px_h: u32) -> Vec<u8> {
    let px_w = px_w.max(1);
    let px_h = px_h.max(1);
    let mut rgb = Vec::with_capacity((px_w * px_h * 3) as usize);
    for y in 0..px_h {
        for x in 0..px_w {
            let sx = (u64::from(x) * u64::from(cover.width) / u64::from(px_w)) as u32;
            let sy = (u64::from(y) * u64::from(cover.height) / u64::from(px_h)) as u32;
            rgb.extend_from_slice(&cover.pixel(sx, sy));
        }
    }
    rgb
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(*chunk.get(1).unwrap_or(&0));
        let b2 = u32::from(*chunk.get(2).unwrap_or(&0));
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64[((n >> 18) & 63) as usize] as char);
        out.push(B64[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(B64[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Builds a kitty graphics protocol sequence that transmits and displays the
/// cover as raw RGB at the cursor.
pub fn kitty_sequence(cover: &CoverImage, px_w: u32, px_h: u32) -> String {
    let px_w = px_w.max(1);
    let px_h = px_h.max(1);
    let encoded = base64_encode(&sample_rgb(cover, px_w, px_h));
    let mut out = String::new();
    let chunks: Vec<&str> = encoded
        .as_bytes()
        .chunks(4096)
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or(""))
        .collect();
    let total = chunks.len();
    for (index, chunk) in chunks.iter().enumerate() {
        let more = index + 1 < total;
        if index == 0 {
            out.push_str(&format!(
                "\x1b_Ga=T,f=24,s={px_w},v={px_h},m={}",
                u8::from(more)
            ));
        } else {
            out.push_str(&format!("\x1b_Gm={}", u8::from(more)));
        }
        out.push(';');
        out.push_str(chunk);
        out.push_str("\x1b\\");
    }
    out
}

pub fn kitty_clear() -> &'static str {
    "\x1b_Ga=d\x1b\\"
}

/// Quantises a channel to one of six levels (6x6x6 = 216 colours).
fn level(value: u8) -> u8 {
    (u32::from(value) * 5 / 255) as u8
}

/// Builds a Sixel sequence for the cover. Colours are quantised to a 216-entry
/// cube; runs are length-encoded with `!<n><char>`.
pub fn sixel_sequence(cover: &CoverImage, px_w: u32, px_h: u32) -> String {
    let px_w = px_w.max(1);
    let px_h = px_h.max(1);
    let rgb = sample_rgb(cover, px_w, px_h);

    let mut indices = vec![0u8; (px_w * px_h) as usize];
    let mut used = [false; 216];
    for (slot, pixel) in indices.iter_mut().zip(rgb.chunks(3)) {
        let index = level(pixel[0]) * 36 + level(pixel[1]) * 6 + level(pixel[2]);
        *slot = index;
        used[index as usize] = true;
    }

    let mut out = String::from("\x1bPq");
    out.push_str(&format!("\"1;1;{px_w};{px_h}"));
    for (index, present) in used.iter().enumerate() {
        if !present {
            continue;
        }
        let index = index as u32;
        let r = (index / 36) % 6;
        let g = (index / 6) % 6;
        let b = index % 6;
        out.push_str(&format!(
            "#{index};2;{};{};{}",
            r * 100 / 5,
            g * 100 / 5,
            b * 100 / 5
        ));
    }

    let bands = px_h.div_ceil(6);
    for band in 0..bands {
        let mut first_color = true;
        for (color, present) in used.iter().enumerate() {
            if !present {
                continue;
            }
            let mut columns: Vec<u8> = Vec::with_capacity(px_w as usize);
            for x in 0..px_w {
                let mut value = 0u8;
                for row in 0..6 {
                    let y = band * 6 + row;
                    if y < px_h && indices[(y * px_w + x) as usize] == color as u8 {
                        value |= 1 << row;
                    }
                }
                columns.push(value);
            }
            while columns.last() == Some(&0) {
                columns.pop();
            }
            if columns.is_empty() {
                continue;
            }
            if !first_color {
                out.push('$');
            }
            first_color = false;
            out.push_str(&format!("#{color}"));
            let mut index = 0;
            while index < columns.len() {
                let value = columns[index];
                let mut run = 1;
                while index + run < columns.len() && columns[index + run] == value {
                    run += 1;
                }
                if run > 3 {
                    out.push_str(&format!("!{run}{}", (63 + value) as char));
                } else {
                    for _ in 0..run {
                        out.push((63 + value) as char);
                    }
                }
                index += run;
            }
        }
        if band + 1 < bands {
            out.push('-');
        }
    }
    out.push_str("\x1b\\");
    out
}

/// Emits the cover using the native protocol once per change. Must be called
/// after `terminal.draw` so the escape sequences land on top of the frame.
pub fn flush<B: Backend + Write>(terminal: &mut Terminal<B>, app: &mut App) -> AppResult<()> {
    if !matches!(app.image_protocol, ImageProtocol::Kitty | ImageProtocol::Sixel) {
        return Ok(());
    }
    let showing = app.view == View::NowPlaying;
    let cover = if showing { app.cover.as_ref() } else { None };
    let Some(cover) = cover else {
        if app.cover_drawn.take().is_some() && app.image_protocol == ImageProtocol::Kitty {
            Write::write_all(terminal.backend_mut(), kitty_clear().as_bytes())?;
            Write::flush(terminal.backend_mut())?;
        }
        return Ok(());
    };
    let size = terminal.size()?;
    let rect = crate::ui::layout::geometry(Rect::new(0, 0, size.width, size.height)).cover;
    if rect.width == 0 || rect.height == 0 {
        return Ok(());
    }
    if app.cover_drawn == Some((app.cover_epoch, rect)) {
        return Ok(());
    }
    let px_w = u32::from(rect.width) * CELL_W;
    let px_h = u32::from(rect.height) * CELL_H;
    let sequence = match app.image_protocol {
        ImageProtocol::Kitty => kitty_sequence(cover, px_w, px_h),
        ImageProtocol::Sixel => sixel_sequence(cover, px_w, px_h),
        _ => return Ok(()),
    };
    let backend = terminal.backend_mut();
    write!(backend, "{}", crossterm::cursor::MoveTo(rect.x, rect.y))?;
    backend.write_all(sequence.as_bytes())?;
    write!(
        backend,
        "{}",
        crossterm::cursor::MoveTo(0, size.height.saturating_sub(1))
    )?;
    Write::flush(backend)?;
    app.cover_drawn = Some((app.cover_epoch, rect));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::CoverImage;

    fn sample() -> CoverImage {
        CoverImage {
            width: 2,
            height: 2,
            pixels: vec![[0, 0, 0], [255, 255, 255], [255, 0, 0], [0, 0, 255]],
        }
    }

    #[test]
    fn average_colour_is_computed() {
        assert_eq!(sample().average_color(), (127, 63, 127));
    }

    #[test]
    fn pixels_are_clamped_to_bounds() {
        let cover = sample();
        assert_eq!(cover.pixel(10, 10), [0, 0, 255]);
        assert_eq!(cover.pixel(0, 0), [0, 0, 0]);
    }

    #[test]
    fn decoding_invalid_bytes_is_an_error() {
        assert!(super::decode(b"not an image").is_err());
    }

    #[test]
    fn base64_matches_known_values() {
        assert_eq!(super::base64_encode(b""), "");
        assert_eq!(super::base64_encode(b"f"), "Zg==");
        assert_eq!(super::base64_encode(b"fo"), "Zm8=");
        assert_eq!(super::base64_encode(b"foo"), "Zm9v");
        assert_eq!(super::base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn kitty_sequence_transmits_raw_rgb() {
        let sequence = super::kitty_sequence(&sample(), 2, 2);
        assert!(sequence.starts_with("\x1b_Ga=T,f=24,s=2,v=2,m=0;"));
        assert!(sequence.ends_with("\x1b\\"));
    }

    #[test]
    fn sixel_sequence_is_well_formed() {
        let sequence = super::sixel_sequence(&sample(), 4, 4);
        assert!(sequence.starts_with("\x1bPq"));
        assert!(sequence.contains("\"1;1;4;4"));
        assert!(sequence.contains('#'));
        assert!(sequence.ends_with("\x1b\\"));
    }

    #[test]
    fn one_pixel_sequences_do_not_panic() {
        assert!(!super::kitty_sequence(&sample(), 0, 0).is_empty());
        assert!(!super::sixel_sequence(&sample(), 1, 1).is_empty());
    }
}
