//! Color handling, mirroring `anxwritter/colors.py`.
//!
//! ANB uses Windows COLORREF integers: `R + G*256 + B*65536` (little-endian
//! BGR, upper byte always 0). A user may supply a color as a COLORREF int, a
//! named color (`"Light Orange"`, `"light_orange"`, ...), a hex string
//! (`"#RRGGBB"`), or a [`Color`] enum variant; [`ColorValue`] unifies those and
//! [`ColorValue::to_colorref`] resolves to the canonical integer.

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};

use crate::enums::Color;

/// Build a COLORREF integer from an `(r, g, b)` triple.
pub const fn rgb_to_colorref(r: u32, g: u32, b: u32) -> u32 {
    r + g * 256 + b * 65536
}

/// The 40 named ANB shading colors as `(normalized_name, colorref)`.
///
/// Names are pre-normalized (lowercase, `-`/space -> `_`) so lookups match the
/// `Color` enum's snake_case values directly.
pub static NAMED_COLORS: &[(&str, u32)] = &[
    ("black", rgb_to_colorref(0, 0, 0)),
    ("brown", rgb_to_colorref(153, 51, 0)),
    ("olive_green", rgb_to_colorref(51, 51, 0)),
    ("dark_green", rgb_to_colorref(0, 51, 0)),
    ("dark_teal", rgb_to_colorref(0, 51, 102)),
    ("dark_blue", rgb_to_colorref(0, 0, 128)),
    ("indigo", rgb_to_colorref(51, 51, 153)),
    ("dark_grey", rgb_to_colorref(51, 51, 51)),
    ("dark_red", rgb_to_colorref(128, 0, 0)),
    ("orange", rgb_to_colorref(255, 102, 0)),
    ("dark_yellow", rgb_to_colorref(128, 128, 0)),
    ("green", rgb_to_colorref(0, 128, 0)),
    ("teal", rgb_to_colorref(0, 128, 128)),
    ("blue", rgb_to_colorref(0, 0, 255)),
    ("blue_grey", rgb_to_colorref(102, 102, 153)),
    ("grey", rgb_to_colorref(128, 128, 128)),
    ("red", rgb_to_colorref(255, 0, 0)),
    ("light_orange", rgb_to_colorref(255, 153, 0)),
    ("lime", rgb_to_colorref(153, 204, 0)),
    ("sea_green", rgb_to_colorref(51, 153, 102)),
    ("aqua", rgb_to_colorref(51, 204, 204)),
    ("light_blue", rgb_to_colorref(51, 102, 255)),
    ("violet", rgb_to_colorref(128, 0, 128)),
    ("light_grey", rgb_to_colorref(153, 153, 153)),
    ("pink", rgb_to_colorref(255, 0, 255)),
    ("gold", rgb_to_colorref(255, 204, 0)),
    ("yellow", rgb_to_colorref(255, 255, 0)),
    ("bright_green", rgb_to_colorref(0, 255, 0)),
    ("turquoise", rgb_to_colorref(0, 255, 255)),
    ("sky_blue", rgb_to_colorref(0, 204, 255)),
    ("plum", rgb_to_colorref(153, 51, 102)),
    ("silver", rgb_to_colorref(192, 192, 192)),
    ("rose", rgb_to_colorref(255, 153, 204)),
    ("tan", rgb_to_colorref(255, 204, 153)),
    ("light_yellow", rgb_to_colorref(255, 255, 153)),
    ("light_green", rgb_to_colorref(204, 255, 204)),
    ("light_turquoise", rgb_to_colorref(204, 255, 255)),
    ("pale_blue", rgb_to_colorref(153, 204, 255)),
    ("lavender", rgb_to_colorref(204, 153, 255)),
    ("white", rgb_to_colorref(255, 255, 255)),
];

/// Canonical key for case/punctuation-insensitive name lookups.
fn normalize_name(name: &str) -> String {
    name.trim().to_lowercase().replace(['-', ' '], "_")
}

/// Resolve a named color to its COLORREF, or `None` if unknown.
pub fn named_to_colorref(name: &str) -> Option<u32> {
    let key = normalize_name(name);
    NAMED_COLORS
        .iter()
        .find(|(n, _)| *n == key)
        .map(|(_, c)| *c)
}

/// A user-supplied color before resolution: an int, a name/hex string, or an
/// enum variant. Deserializes permissively from JSON/YAML scalars.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum ColorValue {
    Int(u32),
    Named(Color),
    Str(String),
}

impl ColorValue {
    /// Resolve to a COLORREF integer, following the same precedence as
    /// `colors.color_to_colorref`: enum/name lookup, int passthrough, hex parse.
    pub fn to_colorref(&self) -> crate::error::Result<u32> {
        match self {
            ColorValue::Int(v) => Ok(*v),
            ColorValue::Named(c) => {
                // The enum's snake_case value is already a normalized key.
                let key = serde_json::to_value(c)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default();
                named_to_colorref(&key)
                    .ok_or_else(|| crate::error::Error::Color(format!("{key:?}")))
            }
            ColorValue::Str(s) => parse_color_str(s),
        }
    }
}

/// Parse a string color: named lookup first, then `#RRGGBB` / `RRGGBB` hex.
pub fn parse_color_str(s: &str) -> crate::error::Result<u32> {
    if let Some(c) = named_to_colorref(s) {
        return Ok(c);
    }
    let hex = s.trim().strip_prefix('#').unwrap_or(s.trim());
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        let r = u32::from_str_radix(&hex[0..2], 16).unwrap();
        let g = u32::from_str_radix(&hex[2..4], 16).unwrap();
        let b = u32::from_str_radix(&hex[4..6], 16).unwrap();
        return Ok(rgb_to_colorref(r, g, b));
    }
    Err(crate::error::Error::Color(s.to_string()))
}

// Custom deserialize so a bare integer, a quoted name, or a hex string all work.
impl<'de> Deserialize<'de> for ColorValue {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;
        impl<'de> de::Visitor<'de> for V {
            type Value = ColorValue;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a COLORREF integer, a named color, or a #RRGGBB hex string")
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<ColorValue, E> {
                Ok(ColorValue::Int(v as u32))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<ColorValue, E> {
                Ok(ColorValue::Int(v as u32))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<ColorValue, E> {
                Ok(ColorValue::Str(v.to_string()))
            }
        }
        deserializer.deserialize_any(V)
    }
}

/// Split a COLORREF into `(r, g, b)`.
pub fn colorref_to_rgb(c: u32) -> (u32, u32, u32) {
    (c & 0xFF, (c >> 8) & 0xFF, (c >> 16) & 0xFF)
}

/// Python `round()` — banker's rounding (round half to even). Matches the
/// component rounding used by the color ramps and width scaling.
fn py_round(x: f64) -> f64 {
    let floor = x.floor();
    let diff = x - floor;
    if (diff - 0.5).abs() < 1e-9 {
        if (floor as i64) % 2 == 0 {
            floor
        } else {
            floor + 1.0
        }
    } else {
        x.round()
    }
}

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f64) -> f64 {
    if c <= 0.0 {
        0.0
    } else if c >= 1.0 {
        1.0
    } else if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Naive component-wise sRGB lerp.
pub fn lerp_rgb(c1: u32, c2: u32, t: f64) -> u32 {
    if t <= 0.0 {
        return c1;
    }
    if t >= 1.0 {
        return c2;
    }
    let (r1, g1, b1) = colorref_to_rgb(c1);
    let (r2, g2, b2) = colorref_to_rgb(c2);
    let mix = |a: u32, b: u32| py_round(a as f64 + (b as f64 - a as f64) * t) as u32;
    rgb_to_colorref(mix(r1, r2), mix(g1, g2), mix(b1, b2))
}

/// Gamma-correct sRGB lerp (round-trip through linear light).
pub fn lerp_rgb_linear(c1: u32, c2: u32, t: f64) -> u32 {
    if t <= 0.0 {
        return c1;
    }
    if t >= 1.0 {
        return c2;
    }
    let (r1, g1, b1) = colorref_to_rgb(c1);
    let (r2, g2, b2) = colorref_to_rgb(c2);
    let mix = |a: u32, b: u32| {
        let la = srgb_to_linear(a as f64 / 255.0);
        let lb = srgb_to_linear(b as f64 / 255.0);
        py_round(linear_to_srgb(la + (lb - la) * t) * 255.0) as u32
    };
    rgb_to_colorref(mix(r1, r2), mix(g1, g2), mix(b1, b2))
}

// colorsys HLS helpers.
fn rgb_to_hls(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let maxc = r.max(g).max(b);
    let minc = r.min(g).min(b);
    let l = (minc + maxc) / 2.0;
    if (minc - maxc).abs() < f64::EPSILON {
        return (0.0, l, 0.0);
    }
    let d = maxc - minc;
    let s = if l <= 0.5 {
        d / (maxc + minc)
    } else {
        d / (2.0 - maxc - minc)
    };
    let rc = (maxc - r) / d;
    let gc = (maxc - g) / d;
    let bc = (maxc - b) / d;
    let mut h = if r == maxc {
        bc - gc
    } else if g == maxc {
        2.0 + rc - bc
    } else {
        4.0 + gc - rc
    };
    h = (h / 6.0).rem_euclid(1.0);
    (h, l, s)
}

fn hls_to_rgb(h: f64, l: f64, s: f64) -> (f64, f64, f64) {
    if s == 0.0 {
        return (l, l, l);
    }
    let m2 = if l <= 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let m1 = 2.0 * l - m2;
    (
        v(m1, m2, h + 1.0 / 3.0),
        v(m1, m2, h),
        v(m1, m2, h - 1.0 / 3.0),
    )
}

fn v(m1: f64, m2: f64, hue: f64) -> f64 {
    let hue = hue.rem_euclid(1.0);
    if hue < 1.0 / 6.0 {
        m1 + (m2 - m1) * hue * 6.0
    } else if hue < 0.5 {
        m2
    } else if hue < 2.0 / 3.0 {
        m1 + (m2 - m1) * (2.0 / 3.0 - hue) * 6.0
    } else {
        m1
    }
}

/// Interpolate via HLS along the shorter hue arc.
pub fn lerp_hsl(c1: u32, c2: u32, t: f64) -> u32 {
    if t <= 0.0 {
        return c1;
    }
    if t >= 1.0 {
        return c2;
    }
    let (r1, g1, b1) = colorref_to_rgb(c1);
    let (r2, g2, b2) = colorref_to_rgb(c2);
    let (h1, l1, s1) = rgb_to_hls(r1 as f64 / 255.0, g1 as f64 / 255.0, b1 as f64 / 255.0);
    let (h2, l2, s2) = rgb_to_hls(r2 as f64 / 255.0, g2 as f64 / 255.0, b2 as f64 / 255.0);
    let mut dh = h2 - h1;
    if dh > 0.5 {
        dh -= 1.0;
    } else if dh < -0.5 {
        dh += 1.0;
    }
    let h = (h1 + dh * t).rem_euclid(1.0);
    let l = l1 + (l2 - l1) * t;
    let s = s1 + (s2 - s1) * t;
    let (r, g, b) = hls_to_rgb(h, l, s);
    rgb_to_colorref(
        py_round(r * 255.0) as u32,
        py_round(g * 255.0) as u32,
        py_round(b * 255.0) as u32,
    )
}

/// Evaluate a multi-stop color ramp at `t` in `[0, 1]`. Stops are evenly spaced.
pub fn interpolate_ramp(ramp: &[u32], t: f64, space: &str) -> u32 {
    let n = ramp.len();
    if n == 0 {
        return 0;
    }
    if n == 1 {
        return ramp[0];
    }
    if t <= 0.0 {
        return ramp[0];
    }
    if t >= 1.0 {
        return ramp[n - 1];
    }
    let seg_len = 1.0 / (n as f64 - 1.0);
    let mut idx = (t / seg_len) as usize;
    if idx >= n - 1 {
        idx = n - 2;
    }
    let local_t = (t - idx as f64 * seg_len) / seg_len;
    let lerp = match space {
        "rgb" => lerp_rgb,
        "hsl" => lerp_hsl,
        _ => lerp_rgb_linear,
    };
    lerp(ramp[idx], ramp[idx + 1], local_t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_lookup_is_case_and_punctuation_insensitive() {
        let want = rgb_to_colorref(255, 153, 0);
        assert_eq!(named_to_colorref("Light Orange"), Some(want));
        assert_eq!(named_to_colorref("light-orange"), Some(want));
        assert_eq!(named_to_colorref("light_orange"), Some(want));
    }

    #[test]
    fn hex_parsing() {
        assert_eq!(
            parse_color_str("#FF0000").unwrap(),
            rgb_to_colorref(255, 0, 0)
        );
        assert_eq!(
            parse_color_str("00FF00").unwrap(),
            rgb_to_colorref(0, 255, 0)
        );
    }

    #[test]
    fn white_and_black_constants() {
        assert_eq!(named_to_colorref("white"), Some(16777215));
        assert_eq!(named_to_colorref("black"), Some(0));
    }

    #[test]
    fn enum_resolves() {
        assert_eq!(
            ColorValue::Named(Color::Red).to_colorref().unwrap(),
            rgb_to_colorref(255, 0, 0)
        );
    }
}
