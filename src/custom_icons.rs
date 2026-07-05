//! Custom icon embedding, mirroring `anxwritter/custom_icons.py`.
//!
//! An image source (path or `data:` URI) is converted to a 24-bit, magenta-keyed
//! BMP (downscale-only to 128px, 1-bit alpha threshold), then zlib-compressed and
//! base64-encoded for the `<CustomImageCollection>`. The produced BMP is valid
//! for i2 but not byte-identical to Pillow's output (the correctness bar is a
//! valid file, not identical bytes).

use std::io::Write;

use base64::Engine;

/// Prefix prepended to emitted custom icon names to avoid clashing with the
/// ANB built-in icon set.
pub const EMITTED_PREFIX: &str = "anxW_";

const MAX_SIZE: u32 = 128;
const MAGENTA: [u8; 3] = [255, 0, 255];

/// The two custom-image kinds.
#[derive(Clone, Copy, PartialEq)]
pub enum IconKind {
    Icon,
    Attribute,
}

/// `<emitted>,Screen,<Kind>` — the `Id` of a `<CustomImage>`.
pub fn composite_key(emitted: &str, kind: IconKind) -> String {
    match kind {
        IconKind::Icon => format!("{emitted},Screen,Icon"),
        IconKind::Attribute => format!("{emitted},Screen,Attribute"),
    }
}

/// Resolve an image source (a `data:` URI or a filesystem path) to raw bytes.
fn load_source(src: &str) -> Result<Vec<u8>, String> {
    if let Some(rest) = src.strip_prefix("data:") {
        let b64 = rest
            .split_once(',')
            .map(|(_, b)| b)
            .ok_or("malformed data: URI")?;
        base64::engine::general_purpose::STANDARD
            .decode(b64.trim())
            .map_err(|e| e.to_string())
    } else {
        std::fs::read(src).map_err(|e| format!("{src}: {e}"))
    }
}

/// Convert an image source to a 24-bit magenta-keyed BMP.
pub fn to_bmp(src: &str) -> Result<Vec<u8>, String> {
    let bytes = load_source(src)?;
    // A ready BMP passes through verbatim (Pillow does the same for 8/24-bit).
    if bytes.starts_with(b"BM") {
        return Ok(bytes);
    }
    let img = image::load_from_memory(&bytes)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    let (w, h) = img.dimensions();
    let maxd = w.max(h).max(1);
    let scale = (MAX_SIZE as f64 / maxd as f64).min(1.0);
    let (nw, nh, resized) = if scale < 1.0 {
        let nw = ((w as f64 * scale).round() as u32).max(1);
        let nh = ((h as f64 * scale).round() as u32).max(1);
        (
            nw,
            nh,
            image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Lanczos3),
        )
    } else {
        (w, h, img)
    };
    let side = nw.max(nh);
    let mut canvas = image::RgbImage::from_pixel(side, side, image::Rgb(MAGENTA));
    let (xo, yo) = ((side - nw) / 2, (side - nh) / 2);
    for y in 0..nh {
        for x in 0..nw {
            let p = resized.get_pixel(x, y).0;
            // Hard 1-bit alpha threshold: opaque pixels overwrite the magenta key.
            if p[3] >= 128 {
                canvas.put_pixel(xo + x, yo + y, image::Rgb([p[0], p[1], p[2]]));
            }
        }
    }
    let mut buf = Vec::new();
    image::codecs::bmp::BmpEncoder::new(&mut buf)
        .encode(canvas.as_raw(), side, side, image::ExtendedColorType::Rgb8)
        .map_err(|e| e.to_string())?;
    Ok(buf)
}

/// `(base64(zlib(bmp, level 9)), uncompressed_len)`.
pub fn payload(bmp: &[u8]) -> (String, u32) {
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(9));
    let _ = enc.write_all(bmp);
    let compressed = enc.finish().unwrap_or_default();
    (
        base64::engine::general_purpose::STANDARD.encode(&compressed),
        bmp.len() as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_roundtrips_to_a_valid_bmp() {
        // 2x2 RGB raw -> we just feed a tiny BMP through passthrough.
        let bmp = b"BM\x00\x00".to_vec();
        let (data, len) = payload(&bmp);
        assert_eq!(len, 4);
        use flate2::read::ZlibDecoder;
        use std::io::Read;
        let compressed = base64::engine::general_purpose::STANDARD
            .decode(data)
            .unwrap();
        let mut d = ZlibDecoder::new(&compressed[..]);
        let mut out = Vec::new();
        d.read_to_end(&mut out).unwrap();
        assert_eq!(&out[..2], b"BM");
    }
}
