//! Public enums, mirroring `anxwritter/enums.py`.
//!
//! Each enum serializes to the same lowercase/snake_case string the Python
//! library uses as its `.value`. Where the Python enum also accepts shorthand
//! aliases (e.g. `->` for an arrow), those are wired up with `#[serde(alias)]`
//! so JSON/YAML configs deserialize identically.
//!
//! The builder is responsible for translating these canonical values into the
//! PascalCase tokens that appear in the `.anx` XML (e.g. `head` -> `ArrowOnHead`).

use serde::{Deserialize, Serialize};

/// How an entity is drawn on the chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Representation {
    Icon,
    ThemeLine,
    EventFrame,
    Box,
    Circle,
    TextBlock,
    Label,
    /// Not yet implemented; falls back to Icon at build time.
    OleObject,
}

/// Direction of a link's arrowhead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArrowStyle {
    #[serde(rename = "head", alias = "->", alias = "ArrowOnHead")]
    ArrowOnHead,
    #[serde(rename = "tail", alias = "<-", alias = "ArrowOnTail")]
    ArrowOnTail,
    #[serde(rename = "both", alias = "<->", alias = "ArrowOnBoth")]
    ArrowOnBoth,
}

/// Data type of an attribute class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeType {
    Text,
    Flag,
    Datetime,
    Number,
}

/// Link connection multiplicity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Multiplicity {
    Multiple,
    Single,
    Directed,
}

/// How a link behaves relative to a theme line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeWiring {
    #[serde(rename = "keep_event")]
    KeepsAtEventHeight,
    #[serde(rename = "return_theme")]
    ReturnsToThemeHeight,
    #[serde(rename = "next_event")]
    GoesToNextEvent,
    #[serde(rename = "no_diversion")]
    NoDiversion,
}

/// Merge / paste behaviour for an attribute class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeBehaviour {
    #[serde(rename = "assign")]
    Assign,
    #[serde(rename = "noop")]
    NoOp,
    #[serde(rename = "add")]
    Add,
    #[serde(rename = "add_space")]
    AddWithSpace,
    #[serde(rename = "add_line_break")]
    AddWithLineBreak,
    #[serde(rename = "max")]
    Max,
    #[serde(rename = "min")]
    Min,
    #[serde(rename = "subtract")]
    Subtract,
    #[serde(rename = "subtract_swap")]
    SubtractSwap,
    #[serde(rename = "or")]
    Or,
    #[serde(rename = "and")]
    And,
    #[serde(rename = "xor")]
    Xor,
}

/// Icon enlargement factor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Enlargement {
    Half,
    Single,
    Double,
    Triple,
    Quadruple,
}

/// Line dash/dot pattern for a strength.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DotStyle {
    #[serde(rename = "solid", alias = "-")]
    Solid,
    #[serde(rename = "dashed", alias = "---")]
    Dashed,
    #[serde(rename = "dash_dot", alias = "-.")]
    DashDot,
    #[serde(rename = "dash_dot_dot", alias = "-..")]
    DashDotDot,
    #[serde(rename = "dotted", alias = "...")]
    Dotted,
}

/// Kind of a legend row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegendItemType {
    Font,
    Text,
    Icon,
    Attribute,
    Line,
    Link,
    Timezone,
    IconFrame,
}

/// Scaling function for data-driven intensity styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntensityScale {
    Linear,
    Log,
    Sqrt,
    Power,
    Quantile,
    Threshold,
}

/// Color space used to interpolate ramps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorSpace {
    Rgb,
    #[default]
    RgbLinear,
    Hsl,
}

/// Policy when an attribute referenced by styling is missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingPolicy {
    Fallback,
    Skip,
    Error,
}

/// The 40 named i2 ANB shading colors, in snake_case (matches Python `Color`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Color {
    Black,
    Brown,
    OliveGreen,
    DarkGreen,
    DarkTeal,
    DarkBlue,
    Indigo,
    DarkGrey,
    DarkRed,
    Orange,
    DarkYellow,
    Green,
    Teal,
    Blue,
    BlueGrey,
    Grey,
    Red,
    LightOrange,
    Lime,
    SeaGreen,
    Aqua,
    LightBlue,
    Violet,
    LightGrey,
    Pink,
    Gold,
    Yellow,
    BrightGreen,
    Turquoise,
    SkyBlue,
    Plum,
    Silver,
    Rose,
    Tan,
    LightYellow,
    LightGreen,
    LightTurquoise,
    PaleBlue,
    Lavender,
    White,
}

/// Introspection for the public config/style enums.
///
/// Each enum exposes its full variant list ([`VARIANTS`](EnumMeta::VARIANTS))
/// and, per variant, the exact lowercase/snake_case string it serializes to
/// ([`as_str`](EnumMeta::as_str)) — the same token that appears in JSON/YAML
/// config. This lets a downstream crate (e.g. an HTTP service building a
/// discovery/`meta` payload) enumerate every accepted value **without
/// hardcoding** it, keeping the payload in lock-step with the emitter.
///
/// The [`crate::discovery`] module collects these into ready-to-serialize
/// tables.
pub trait EnumMeta: Copy + 'static {
    /// Every variant, in declaration order.
    const VARIANTS: &'static [Self];

    /// This variant's serialized string value (the `.value` of the Python enum).
    fn as_str(self) -> &'static str;

    /// The serialized string values of all variants, in declaration order.
    fn values() -> Vec<&'static str> {
        Self::VARIANTS.iter().map(|v| v.as_str()).collect()
    }
}

/// Implement [`EnumMeta`] for an enum by pairing each variant with the exact
/// string it serializes to. A test (`enum_meta_matches_serde`) asserts these
/// strings equal the serde serialization, so the two can never drift.
macro_rules! enum_meta {
    ($ty:ty { $($variant:ident => $s:literal),+ $(,)? }) => {
        impl EnumMeta for $ty {
            const VARIANTS: &'static [$ty] = &[$(<$ty>::$variant),+];
            fn as_str(self) -> &'static str {
                match self { $(<$ty>::$variant => $s),+ }
            }
        }
    };
}

enum_meta!(Representation {
    Icon => "icon", ThemeLine => "theme_line", EventFrame => "event_frame",
    Box => "box", Circle => "circle", TextBlock => "text_block",
    Label => "label", OleObject => "ole_object",
});
enum_meta!(ArrowStyle {
    ArrowOnHead => "head", ArrowOnTail => "tail", ArrowOnBoth => "both",
});
enum_meta!(AttributeType {
    Text => "text", Flag => "flag", Datetime => "datetime", Number => "number",
});
enum_meta!(Multiplicity {
    Multiple => "multiple", Single => "single", Directed => "directed",
});
enum_meta!(ThemeWiring {
    KeepsAtEventHeight => "keep_event", ReturnsToThemeHeight => "return_theme",
    GoesToNextEvent => "next_event", NoDiversion => "no_diversion",
});
enum_meta!(MergeBehaviour {
    Assign => "assign", NoOp => "noop", Add => "add", AddWithSpace => "add_space",
    AddWithLineBreak => "add_line_break", Max => "max", Min => "min",
    Subtract => "subtract", SubtractSwap => "subtract_swap", Or => "or",
    And => "and", Xor => "xor",
});
enum_meta!(Enlargement {
    Half => "half", Single => "single", Double => "double",
    Triple => "triple", Quadruple => "quadruple",
});
enum_meta!(DotStyle {
    Solid => "solid", Dashed => "dashed", DashDot => "dash_dot",
    DashDotDot => "dash_dot_dot", Dotted => "dotted",
});
enum_meta!(LegendItemType {
    Font => "font", Text => "text", Icon => "icon", Attribute => "attribute",
    Line => "line", Link => "link", Timezone => "timezone", IconFrame => "icon_frame",
});
enum_meta!(IntensityScale {
    Linear => "linear", Log => "log", Sqrt => "sqrt", Power => "power",
    Quantile => "quantile", Threshold => "threshold",
});
enum_meta!(ColorSpace {
    Rgb => "rgb", RgbLinear => "rgb_linear", Hsl => "hsl",
});
enum_meta!(MissingPolicy {
    Fallback => "fallback", Skip => "skip", Error => "error",
});
enum_meta!(Color {
    Black => "black", Brown => "brown", OliveGreen => "olive_green",
    DarkGreen => "dark_green", DarkTeal => "dark_teal", DarkBlue => "dark_blue",
    Indigo => "indigo", DarkGrey => "dark_grey", DarkRed => "dark_red",
    Orange => "orange", DarkYellow => "dark_yellow", Green => "green",
    Teal => "teal", Blue => "blue", BlueGrey => "blue_grey", Grey => "grey",
    Red => "red", LightOrange => "light_orange", Lime => "lime",
    SeaGreen => "sea_green", Aqua => "aqua", LightBlue => "light_blue",
    Violet => "violet", LightGrey => "light_grey", Pink => "pink", Gold => "gold",
    Yellow => "yellow", BrightGreen => "bright_green", Turquoise => "turquoise",
    SkyBlue => "sky_blue", Plum => "plum", Silver => "silver", Rose => "rose",
    Tan => "tan", LightYellow => "light_yellow", LightGreen => "light_green",
    LightTurquoise => "light_turquoise", PaleBlue => "pale_blue",
    Lavender => "lavender", White => "white",
});

#[cfg(test)]
mod tests {
    use super::*;

    /// The hand-written `as_str` strings must equal serde's serialization for
    /// every variant, so the discovery tables can never drift from the wire.
    #[test]
    fn enum_meta_matches_serde() {
        fn check<T: EnumMeta + serde::Serialize>() {
            for v in T::VARIANTS {
                let serded = serde_json::to_value(v).unwrap();
                assert_eq!(serded.as_str().unwrap(), v.as_str());
            }
        }
        check::<Representation>();
        check::<ArrowStyle>();
        check::<AttributeType>();
        check::<Multiplicity>();
        check::<ThemeWiring>();
        check::<MergeBehaviour>();
        check::<Enlargement>();
        check::<DotStyle>();
        check::<LegendItemType>();
        check::<IntensityScale>();
        check::<ColorSpace>();
        check::<MissingPolicy>();
        check::<Color>();
    }
}
