//! Card pictures for the PDFs: read, cropped or fitted, scaled down and re-encoded as JPEG,
//! which PDFs embed as is.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context as _, Result};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::{self, FilterType};
use image::{DynamicImage, RgbImage};

use crate::model::Card;

/// How artwork goes in its `w_mm` × `h_mm` box.
#[derive(Clone, Copy)]
pub(super) enum Fit {
    /// Fills the box, cropping the edges; landscape pictures (climaxes) are turned upright.
    Fill,
    /// Whole picture, as large as fits.
    Contain,
}

/// Artwork of each card, fitted to `w_mm` × `h_mm` and scaled down to `dpi`, by card key.
/// Cards without a readable picture are left out.
pub(super) fn prepare_art(
    cards: &[&Card],
    w_mm: f32,
    h_mm: f32,
    dpi: f32,
    fit: Fit,
) -> HashMap<String, Jpeg> {
    std::thread::scope(|s| {
        let jobs: Vec<_> = cards
            .iter()
            .filter_map(|card| Some((card.key.clone(), card.image.as_deref()?)))
            .map(|(key, path)| s.spawn(move || (key, Jpeg::card(path, w_mm, h_mm, dpi, fit))))
            .collect();
        jobs.into_iter()
            .filter_map(|job| job.join().ok())
            .filter_map(|(key, jpeg)| Some((key, jpeg.ok()?)))
            .collect()
    })
}

pub(super) struct Jpeg {
    pub(super) data: Vec<u8>,
    pub(super) width: u32,
    pub(super) height: u32,
}

impl Jpeg {
    pub(super) fn card(path: &Path, w_mm: f32, h_mm: f32, dpi: f32, fit: Fit) -> Result<Self> {
        let image = image::open(path).with_context(|| format!("reading {}", path.display()))?;
        let mut rgb = flatten(image);
        let px = |mm: f32| (mm / 25.4 * dpi).round() as u32;
        let (max_w, max_h) = (px(w_mm), px(h_mm));
        match fit {
            Fit::Fill => {
                if rgb.width() > rgb.height() {
                    rgb = imageops::rotate90(&rgb);
                }
                // Crop to the printed shape, keeping the middle.
                let target = w_mm / h_mm;
                let (w, h) = (rgb.width(), rgb.height());
                let (cw, ch) = if w as f32 / h as f32 > target {
                    ((h as f32 * target).round() as u32, h)
                } else {
                    (w, (w as f32 / target).round() as u32)
                };
                rgb = imageops::crop_imm(&rgb, (w - cw) / 2, (h - ch) / 2, cw, ch).to_image();
                if rgb.width() > max_w {
                    rgb = imageops::resize(&rgb, max_w, max_h, FilterType::Lanczos3);
                }
            }
            Fit::Contain => {
                let scale =
                    (max_w as f32 / rgb.width() as f32).min(max_h as f32 / rgb.height() as f32);
                if scale < 1.0 {
                    let w = (rgb.width() as f32 * scale).round().max(1.0) as u32;
                    let h = (rgb.height() as f32 * scale).round().max(1.0) as u32;
                    rgb = imageops::resize(&rgb, w, h, FilterType::Lanczos3);
                }
            }
        }
        let mut data = Vec::new();
        JpegEncoder::new_with_quality(&mut data, 90).encode_image(&rgb)?;
        Ok(Self {
            data,
            width: rgb.width(),
            height: rgb.height(),
        })
    }
}

/// RGB on a white background.
fn flatten(image: DynamicImage) -> RgbImage {
    if !image.color().has_alpha() {
        return image.to_rgb8();
    }
    let rgba = image.to_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let over = |c: u8| ((c as u32 * a as u32 + 255 * (255 - a as u32)) / 255) as u8;
        image::Rgb([over(r), over(g), over(b)])
    })
}
