// CorelDRAW X7 ignores preserveAspectRatio on imported SVG images and stretches every picture to its box.
// So the uploaded photo and signature are given the exact shape of their box here, and the template draws
// them with preserveAspectRatio="none": the result is the same in CorelDRAW and in a browser.
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use image::{imageops, imageops::FilterType, DynamicImage, GenericImageView, RgbaImage};
use std::io::Cursor;

// Card units are 1/1000 inch, so a box's size in units is its size in pixels at 1000 dpi - plenty for printing
const MAX_PIXELS_PER_UNIT: f64 = 1.0;

fn decode(data_url: &str) -> Option<DynamicImage> {
    let (_, payload) = data_url.split_once("base64,")?;
    let bytes = BASE64.decode(payload.trim()).ok()?;
    image::load_from_memory(&bytes).ok()
}

fn to_data_url(img: &DynamicImage, as_jpeg: bool) -> Option<String> {
    let mut out = Cursor::new(Vec::new());
    if as_jpeg {
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 92);
        img.to_rgb8().write_with_encoder(encoder).ok()?;
        Some(format!("data:image/jpeg;base64,{}", BASE64.encode(out.into_inner())))
    } else {
        img.write_to(&mut out, image::ImageFormat::Png).ok()?;
        Some(format!("data:image/png;base64,{}", BASE64.encode(out.into_inner())))
    }
}

// Pixel size for a box, never enlarging the source beyond what it has
fn target_size(box_w: f64, box_h: f64, available_w: u32, available_h: u32) -> (u32, u32) {
    let scale = (available_w as f64 / box_w).min(available_h as f64 / box_h).min(MAX_PIXELS_PER_UNIT);
    (((box_w * scale).round() as u32).max(1), ((box_h * scale).round() as u32).max(1))
}

// Like preserveAspectRatio="xMidYMid slice": centre-crop to the box shape (ID photo)
pub fn crop_to_box(img: &DynamicImage, box_w: f64, box_h: f64) -> DynamicImage {
    let (w, h) = img.dimensions();
    let ratio = box_w / box_h;
    let (crop_w, crop_h) = if w as f64 / h as f64 > ratio {
        (((h as f64 * ratio).round() as u32).clamp(1, w), h)
    } else {
        (w, ((w as f64 / ratio).round() as u32).clamp(1, h))
    };
    let cropped = img.crop_imm((w - crop_w) / 2, (h - crop_h) / 2, crop_w, crop_h);
    let (out_w, out_h) = target_size(box_w, box_h, crop_w, crop_h);
    cropped.resize_exact(out_w, out_h, FilterType::Lanczos3)
}

// Like preserveAspectRatio="xMidYMid meet": fit inside the box shape, centred on transparent padding (signature)
pub fn pad_to_box(img: &DynamicImage, box_w: f64, box_h: f64) -> DynamicImage {
    let (w, h) = img.dimensions();
    // Source image plus the padding it needs, then scaled down to at most 1000 dpi
    let padded_w = w.max((h as f64 * box_w / box_h).ceil() as u32);
    let padded_h = h.max((w as f64 * box_h / box_w).ceil() as u32);
    let (canvas_w, canvas_h) = target_size(box_w, box_h, padded_w, padded_h);
    let scale = (canvas_w as f64 / w as f64).min(canvas_h as f64 / h as f64);
    let fit_w = ((w as f64 * scale).round() as u32).clamp(1, canvas_w);
    let fit_h = ((h as f64 * scale).round() as u32).clamp(1, canvas_h);
    let fitted = img.resize_exact(fit_w, fit_h, FilterType::Lanczos3).to_rgba8();
    let mut canvas = RgbaImage::new(canvas_w, canvas_h);
    imageops::overlay(&mut canvas, &fitted, ((canvas_w - fit_w) / 2) as i64, ((canvas_h - fit_h) / 2) as i64);
    DynamicImage::ImageRgba8(canvas)
}

// True when some pixel is not fully opaque (e.g. a photo with its background removed)
fn has_transparency(img: &DynamicImage) -> bool {
    img.color().has_alpha() && img.to_rgba8().pixels().any(|p| p.0[3] < 255)
}

// ID photo for a slice box: JPEG for ordinary photos (small files), PNG when the photo has transparent
// parts - JPEG cannot store transparency and would turn those parts black. Unreadable input is passed through.
pub fn photo_for_box(data_url: &str, box_w: f64, box_h: f64) -> String {
    decode(data_url)
        .and_then(|img| {
            let cropped = crop_to_box(&img, box_w, box_h);
            to_data_url(&cropped, !has_transparency(&cropped))
        })
        .unwrap_or_else(|| data_url.to_string())
}

// Signature for a meet box, as a PNG data URL (keeps transparency); unreadable input is passed through unchanged
pub fn signature_for_box(data_url: &str, box_w: f64, box_h: f64) -> String {
    decode(data_url)
        .and_then(|img| to_data_url(&pad_to_box(&img, box_w, box_h), false))
        .unwrap_or_else(|| data_url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ratio(img: &DynamicImage) -> f64 {
        img.width() as f64 / img.height() as f64
    }

    #[test]
    fn photo_is_cropped_to_box_shape() {
        // Portrait phone photo into the ~square ID picture box
        let photo = DynamicImage::new_rgb8(3000, 4000);
        let out = crop_to_box(&photo, 972.0, 982.0);
        assert!((ratio(&out) - 972.0 / 982.0).abs() < 0.01);
        assert_eq!((out.width(), out.height()), (972, 982)); // downscaled to 1000 dpi
    }

    #[test]
    fn small_photo_is_not_enlarged() {
        let out = crop_to_box(&DynamicImage::new_rgb8(400, 300), 972.0, 982.0);
        assert!(out.width() <= 400 && out.height() <= 300);
        assert!((ratio(&out) - 972.0 / 982.0).abs() < 0.02);
    }

    #[test]
    fn signature_is_padded_to_box_shape() {
        // Tall-ish signature scan into the wide 1125 x 250 signature box
        let out = pad_to_box(&DynamicImage::new_rgba8(600, 300), 1125.0, 250.0);
        assert!((ratio(&out) - 1125.0 / 250.0).abs() < 0.02);
        assert_eq!(out.height(), 250);
    }

    fn png_data_url(img: &DynamicImage) -> String {
        to_data_url(img, false).unwrap()
    }

    #[test]
    fn transparent_photo_keeps_its_transparency() {
        // Background-removed photo: transparent everywhere except an opaque square in the middle
        let mut rgba = RgbaImage::new(400, 400);
        for (x, y, pixel) in rgba.enumerate_pixels_mut() {
            if (150..250).contains(&x) && (150..250).contains(&y) {
                *pixel = image::Rgba([40, 40, 40, 255]);
            }
        }
        let out = photo_for_box(&png_data_url(&DynamicImage::ImageRgba8(rgba)), 972.0, 982.0);
        assert!(out.starts_with("data:image/png;base64,"), "transparent photo must stay PNG");
        let decoded = decode(&out).unwrap().to_rgba8();
        assert_eq!(decoded.get_pixel(0, 0).0[3], 0, "corner must still be transparent, not black");
    }

    #[test]
    fn opaque_photo_becomes_jpeg() {
        let opaque = DynamicImage::ImageRgba8(RgbaImage::from_pixel(400, 400, image::Rgba([200, 180, 160, 255])));
        assert!(photo_for_box(&png_data_url(&opaque), 972.0, 982.0).starts_with("data:image/jpeg;base64,"));
    }

    #[test]
    fn unreadable_image_passes_through() {
        assert_eq!(photo_for_box("{{not an image}}", 972.0, 982.0), "{{not an image}}");
        assert_eq!(signature_for_box("", 1125.0, 250.0), "");
    }
}
