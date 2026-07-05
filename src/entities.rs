//! Entity representation types, mirroring `anxwritter/entities.py`.
//!
//! Python uses dataclass inheritance from `_BaseEntity`; Rust has no struct
//! inheritance, so the shared fields live in [`EntityCommon`] and each concrete
//! type flattens it via `#[serde(flatten)]`. In JSON/YAML every field is flat on
//! the entity object (e.g. `{id, type, label, color, attributes}`), exactly as
//! upstream. All fields are optional — the builder supplies i2's emission
//! defaults (line widths, sizes, fills) noted in the doc comments.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

use crate::color::ColorValue;
use crate::models::{Card, Font, Frame, GradeRef, Show, TimeZone};
use crate::value::AttrValue;

/// Fields shared by every entity representation (the `_BaseEntity` payload).
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EntityCommon {
    pub id: String,
    pub r#type: String,
    pub label: Option<String>,
    pub date: Option<String>,
    pub time: Option<String>,
    pub description: Option<String>,
    pub ordered: Option<bool>,
    pub strength: Option<String>,
    pub grade_one: Option<GradeRef>,
    pub grade_two: Option<GradeRef>,
    pub grade_three: Option<GradeRef>,
    pub x: Option<i64>,
    pub y: Option<i64>,
    #[serde(default)]
    pub attributes: IndexMap<String, AttrValue>,
    #[serde(default)]
    pub cards: Vec<Card>,
    #[serde(default)]
    pub label_font: Font,
    pub timezone: Option<TimeZone>,
    pub source_ref: Option<String>,
    pub source_type: Option<String>,
    #[serde(default)]
    pub show: Show,
    pub background: Option<bool>,
    pub datetime_description: Option<String>,
    pub show_datetime_description: Option<bool>,
    pub datetime_format: Option<String>,
    pub sub_text_width: Option<f64>,
    pub use_sub_text_width: Option<bool>,
    pub semantic_type: Option<String>,
}

/// Icon entity — the most common representation. `color` is the icon shading
/// color; `icon` overrides the type's default icon for this instance.
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Icon {
    #[serde(flatten)]
    pub common: EntityCommon,
    pub color: Option<ColorValue>,
    pub icon: Option<String>,
    #[serde(default)]
    pub frame: Frame,
    pub text_x: Option<i64>,
    pub text_y: Option<i64>,
    pub enlargement: Option<String>,
}

/// Box entity — rectangle. Builder defaults: `bg_color` white, `filled` false,
/// `line_width` 1, `width`/`height` 100.
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Box {
    #[serde(flatten)]
    pub common: EntityCommon,
    pub bg_color: Option<ColorValue>,
    pub filled: Option<bool>,
    pub line_color: Option<ColorValue>,
    pub line_width: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub depth: Option<i64>,
}

/// Circle entity. Builder defaults: `bg_color` white, `filled` true,
/// `line_width` 1, `diameter` 138, `autosize` false.
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Circle {
    #[serde(flatten)]
    pub common: EntityCommon,
    pub bg_color: Option<ColorValue>,
    pub filled: Option<bool>,
    pub line_width: Option<i64>,
    pub diameter: Option<i64>,
    pub autosize: Option<bool>,
}

/// ThemeLine entity — a horizontal band. Builder default `line_width` 3.
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeLine {
    #[serde(flatten)]
    pub common: EntityCommon,
    pub shade_color: Option<ColorValue>,
    pub line_color: Option<ColorValue>,
    pub line_width: Option<i64>,
    #[serde(default)]
    pub frame: Frame,
    pub enlargement: Option<String>,
    pub icon: Option<String>,
}

/// EventFrame entity — a time-bounded region. Builder defaults: `bg_color`
/// white, `filled` true, `line_width` 1.
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EventFrame {
    #[serde(flatten)]
    pub common: EntityCommon,
    pub shade_color: Option<ColorValue>,
    pub bg_color: Option<ColorValue>,
    pub filled: Option<bool>,
    pub line_color: Option<ColorValue>,
    pub line_width: Option<i64>,
    pub enlargement: Option<String>,
    pub icon: Option<String>,
}

/// TextBlock entity — a free-standing text box. Builder defaults: `bg_color`
/// white, `filled` true, `line_width` 1, `alignment` centre, `width` 138,
/// `height` 79.
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextBlock {
    #[serde(flatten)]
    pub common: EntityCommon,
    pub bg_color: Option<ColorValue>,
    pub filled: Option<bool>,
    pub line_color: Option<ColorValue>,
    pub line_width: Option<i64>,
    pub alignment: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
}

/// Label entity — a transparent text overlay (rendered via TextBlock XML).
/// Builder defaults: `filled` true, `line_width` 1, `alignment` centre,
/// `width` 100, `height` 39.
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Label {
    #[serde(flatten)]
    pub common: EntityCommon,
    pub bg_color: Option<ColorValue>,
    pub filled: Option<bool>,
    pub line_color: Option<ColorValue>,
    pub line_width: Option<i64>,
    pub alignment: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
}

/// Any entity representation. Mirrors the `entities.{icons,boxes,...}` groups in
/// the input schema; the active variant determines the i2 representation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Entity {
    Icon(Icon),
    Box(Box),
    Circle(Circle),
    ThemeLine(ThemeLine),
    EventFrame(EventFrame),
    TextBlock(TextBlock),
    Label(Label),
}

impl Entity {
    /// Borrow the shared fields regardless of representation.
    pub fn common(&self) -> &EntityCommon {
        match self {
            Entity::Icon(e) => &e.common,
            Entity::Box(e) => &e.common,
            Entity::Circle(e) => &e.common,
            Entity::ThemeLine(e) => &e.common,
            Entity::EventFrame(e) => &e.common,
            Entity::TextBlock(e) => &e.common,
            Entity::Label(e) => &e.common,
        }
    }
}

/// A borrowed view over an entity living in one of the typed data groups. The
/// resolve and emit passes operate on these so entities are never cloned into an
/// owned `Entity` — peak memory holds the input once, not a second copy.
#[derive(Clone, Copy)]
pub enum EntityRef<'a> {
    Icon(&'a Icon),
    Box(&'a Box),
    Circle(&'a Circle),
    ThemeLine(&'a ThemeLine),
    EventFrame(&'a EventFrame),
    TextBlock(&'a TextBlock),
    Label(&'a Label),
}

impl<'a> EntityRef<'a> {
    /// Borrow the shared fields regardless of representation.
    pub fn common(&self) -> &'a EntityCommon {
        match self {
            EntityRef::Icon(e) => &e.common,
            EntityRef::Box(e) => &e.common,
            EntityRef::Circle(e) => &e.common,
            EntityRef::ThemeLine(e) => &e.common,
            EntityRef::EventFrame(e) => &e.common,
            EntityRef::TextBlock(e) => &e.common,
            EntityRef::Label(e) => &e.common,
        }
    }
}
