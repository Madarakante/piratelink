//! Artwork ingestion.
//!
//! SteamGridDB serves multi-megabyte PNGs. We re-encode once at ingest instead of
//! on every render: the shelf only ever needs 600 px of cover, and a 1.9 MB PNG
//! becomes a ~120 KB WebP without touching the pixels.
//!
//! Pixels are *not* upscaled and aspect ratio is preserved; `max_width` is a
//! ceiling, not a target.

use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat};

/// The shelf renders covers at ~300 CSS px, so 600 covers 2× displays.
pub const COVER_MAX_WIDTH: u32 = 600;

/// Heroes come in 1920×620 and 3840×1240; the wider ceiling keeps the 4K copy
/// intact for large or high-DPI windows instead of throwing the pixels away.
pub const HERO_MAX_WIDTH: u32 = 3840;

/// Decodes `bytes`, downscales if needed, and writes the result into `dir` as
/// `<stem>.<ext>`. Returns the written path.
///
/// Format preference is WebP (lossless, alpha-safe), then JPEG, then PNG. The
/// fallbacks exist because encoder availability depends on how the `image` crate
/// was compiled; a missing WebP encoder must not fail an artwork fetch.
pub fn store(bytes: &[u8], dir: &Path, stem: &str, max_width: u32) -> Result<PathBuf, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("unsupported image: {e}"))?;
    let img = downscale(img, max_width);

    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create art cache: {e}"))?;

    let webp = dir.join(format!("{stem}.webp"));
    if img.save_with_format(&webp, ImageFormat::WebP).is_ok() {
        return Ok(webp);
    }
    let _ = std::fs::remove_file(&webp);

    let jpeg = dir.join(format!("{stem}.jpg"));
    if img.to_rgb8().save_with_format(&jpeg, ImageFormat::Jpeg).is_ok() {
        return Ok(jpeg);
    }
    let _ = std::fs::remove_file(&jpeg);

    let png = dir.join(format!("{stem}.png"));
    img.save_with_format(&png, ImageFormat::Png)
        .map_err(|e| format!("could not encode artwork: {e}"))?;
    Ok(png)
}

fn downscale(img: DynamicImage, max_width: u32) -> DynamicImage {
    if img.width() <= max_width {
        return img;
    }
    let height = (img.height() as f64 * (max_width as f64 / img.width() as f64)).round() as u32;
    img.resize_exact(max_width, height.max(1), FilterType::Lanczos3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        let mut img = RgbImage::new(width, height);
        for (x, y, px) in img.enumerate_pixels_mut() {
            *px = Rgb([(x % 256) as u8, (y % 256) as u8, 90]);
        }
        let mut out = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn downscales_to_the_ceiling_and_keeps_aspect() {
        let dir = tempfile::tempdir().unwrap();
        let path = store(&png_bytes(1200, 1800), dir.path(), "1-cover", 600).unwrap();

        let written = image::open(&path).unwrap();
        assert_eq!(written.width(), 600);
        assert_eq!(written.height(), 900, "aspect ratio must be preserved");
        assert!(path.extension().is_some());
    }

    #[test]
    fn does_not_upscale_small_images() {
        let dir = tempfile::tempdir().unwrap();
        let path = store(&png_bytes(300, 450), dir.path(), "2-cover", 600).unwrap();
        let written = image::open(&path).unwrap();
        assert_eq!((written.width(), written.height()), (300, 450));
    }

    #[test]
    fn reencode_is_smaller_than_the_source_png() {
        let dir = tempfile::tempdir().unwrap();
        // Smooth gradient-ish content: PNG is inefficient here, so any lossy or
        // lossless re-encode should beat it.
        let source = png_bytes(900, 1350);
        let path = store(&source, dir.path(), "3-cover", 600).unwrap();

        let out = std::fs::metadata(&path).unwrap().len();
        assert!(out > 1024, "output suspiciously small: {out}");
        assert!(
            out < source.len() as u64,
            "expected {out} bytes to beat the {} byte PNG",
            source.len()
        );
    }

    #[test]
    fn overwrites_on_refetch() {
        let dir = tempfile::tempdir().unwrap();
        let first = store(&png_bytes(800, 1200), dir.path(), "4-cover", 600).unwrap();
        let second = store(&png_bytes(400, 600), dir.path(), "4-cover", 600).unwrap();
        assert_eq!(first, second, "same stem must reuse the same path");
        assert_eq!(image::open(&second).unwrap().width(), 400);
    }

    #[test]
    fn rejects_non_image_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let err = store(b"not an image", dir.path(), "5-cover", 600).unwrap_err();
        assert!(err.contains("unsupported image"), "got {err}");
    }

    #[test]
    fn creates_missing_directories() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("cache").join("art");
        let path = store(&png_bytes(200, 300), &nested, "6-cover", 600).unwrap();
        assert!(path.starts_with(&nested));
        assert!(path.is_file());
    }
}
