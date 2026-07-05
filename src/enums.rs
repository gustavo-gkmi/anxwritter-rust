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
