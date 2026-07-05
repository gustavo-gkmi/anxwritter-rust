//! Chart data models, mirroring `anxwritter/models.py`.
//!
//! These are the typed inputs a user (or a deserialized JSON/YAML config)
//! provides. Almost every field is optional: `None` means "unset", and the
//! builder applies i2's emission defaults. Colour-bearing fields use
//! [`ColorValue`] (int / named / hex); "enum-or-free-string" fields that the
//! Python API leaves as `Optional[str]` are kept as `Option<String>` and
//! normalized later, matching upstream leniency.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

use crate::color::ColorValue;
use crate::enums::{AttributeType, DotStyle, MergeBehaviour};
use crate::value::AttrValue;

/// A grade reference: a 0-based index into a grade collection, or a grade name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GradeRef {
    Index(i64),
    Name(String),
}

/// An intensity domain: an explicit `[min, max]` range or a keyword (`robust`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IntensityDomain {
    Range([f64; 2]),
    Keyword(String),
}

// ── Shared styling shapes ───────────────────────────────────────────────────

/// Font styling (`FaceName`, `PointSize`, colours, weight flags).
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Font {
    pub name: Option<String>,
    pub size: Option<i64>,
    pub color: Option<ColorValue>,
    pub bg_color: Option<ColorValue>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub strikeout: Option<bool>,
    pub underline: Option<bool>,
}

/// Frame highlight border (`FrameStyle`).
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Frame {
    pub color: Option<ColorValue>,
    pub margin: Option<i64>,
    pub visible: Option<bool>,
}

/// Sub-item visibility flags (`SubItemVisibility`).
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Show {
    pub description: Option<bool>,
    pub grades: Option<bool>,
    pub label: Option<bool>,
    pub date: Option<bool>,
    pub source_ref: Option<bool>,
    pub source_type: Option<bool>,
    pub pin: Option<bool>,
}

// ── Settings sub-blocks ─────────────────────────────────────────────────────

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChartCfg {
    pub bg_color: Option<ColorValue>,
    pub bg_filled: Option<bool>,
    pub label_merge_rule: Option<String>,
    pub icon_quality: Option<String>,
    pub rigorous: Option<bool>,
    pub id_reference_linking: Option<bool>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewCfg {
    pub show_pages_boundaries: Option<bool>,
    pub show_all: Option<bool>,
    pub hidden_items: Option<String>,
    pub cover_sheet_on_open: Option<bool>,
    pub time_bar: Option<bool>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GridCfg {
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub snap: Option<bool>,
    pub visible: Option<bool>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WiringCfg {
    pub distance_far: Option<f64>,
    pub distance_near: Option<f64>,
    pub height: Option<f64>,
    pub spacing: Option<f64>,
    pub use_height_for_theme_icon: Option<bool>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LinksCfg {
    pub spacing: Option<f64>,
    pub use_default_spacing_when_dragging: Option<bool>,
    pub blank_labels: Option<bool>,
    pub sum_numeric_labels: Option<bool>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeCfg {
    pub default_date: Option<String>,
    pub default_datetime: Option<String>,
    pub tick_rate: Option<f64>,
    pub local_tz: Option<bool>,
    pub hide_matching_tz_format: Option<bool>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SummaryCfg {
    pub title: Option<String>,
    pub subject: Option<String>,
    pub author: Option<String>,
    pub keywords: Option<String>,
    pub category: Option<String>,
    pub comments: Option<String>,
    pub template: Option<String>,
    pub created: Option<String>,
    pub revision: Option<i64>,
    pub edit_time: Option<i64>,
    pub last_print: Option<String>,
    pub last_save: Option<String>,
    #[serde(default)]
    pub custom_properties: Vec<CustomProperty>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LegendCfg {
    pub show: Option<bool>,
    pub x: Option<i64>,
    pub y: Option<i64>,
    pub arrange: Option<String>,
    pub valign: Option<String>,
    pub halign: Option<String>,
    #[serde(default)]
    pub font: Font,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeoMapCfg {
    pub attribute_name: Option<String>,
    /// `position`, `latlon`, or `both` (default `both`).
    pub mode: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub spread_radius: Option<i64>,
    pub data: Option<IndexMap<String, Vec<f64>>>,
    pub data_file: Option<String>,
    pub accent_insensitive: Option<bool>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IconRule {
    /// `attribute` (default) or `id`.
    pub r#match: Option<String>,
    pub attribute_name: Option<String>,
    pub r#type: Option<String>,
    #[serde(default)]
    pub mapping: IndexMap<String, String>,
    pub default: Option<String>,
    pub default_when_absent: Option<String>,
    pub strict_match: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IconMapCfg {
    pub rules: Vec<IconRule>,
}

// ── Data-driven link styling ────────────────────────────────────────────────

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CategoricalStyleCfg {
    pub line_color: Option<ColorValue>,
    pub line_width: Option<i64>,
    pub strength: Option<String>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IntensityWidthCfg {
    pub attribute: Option<String>,
    pub scale: Option<String>,
    pub domain: Option<IntensityDomain>,
    pub clip: Option<bool>,
    pub power: Option<f64>,
    pub range: Option<Vec<i64>>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IntensityColorCfg {
    pub attribute: Option<String>,
    pub scale: Option<String>,
    pub domain: Option<IntensityDomain>,
    pub clip: Option<bool>,
    pub power: Option<f64>,
    pub ramp: Option<Vec<ColorValue>>,
    /// `rgb`, `rgb_linear` (default), or `hsl`.
    pub space: Option<String>,
    pub diverging: Option<bool>,
    pub midpoint: Option<f64>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IntensityCfg {
    pub attribute: Option<String>,
    pub scale: Option<String>,
    pub domain: Option<IntensityDomain>,
    pub clip: Option<bool>,
    pub missing: Option<String>,
    pub legend: Option<bool>,
    pub legend_count: Option<i64>,
    pub decimal_separator: Option<String>,
    pub thousand_separator: Option<String>,
    pub width: Option<IntensityWidthCfg>,
    pub color: Option<IntensityColorCfg>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CategoricalCfg {
    pub attribute: Option<String>,
    #[serde(default)]
    pub styles: IndexMap<String, CategoricalStyleCfg>,
    pub default: Option<CategoricalStyleCfg>,
    pub missing: Option<String>,
    pub case_sensitive: Option<bool>,
    pub accent_insensitive: Option<bool>,
    pub legend: Option<bool>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LinkStylingCfg {
    pub intensity: Option<IntensityCfg>,
    pub categorical: Option<CategoricalCfg>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StylingCfg {
    pub links: Option<LinkStylingCfg>,
}

// ── Display synthesizers ────────────────────────────────────────────────────

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplaySource {
    pub attribute: Option<String>,
    pub alias: Option<String>,
    pub missing: Option<String>,
    pub placeholder: Option<String>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayAttribute {
    pub key: Option<String>,
    pub attribute_name: Option<String>,
    /// `entity`, `link`, or `both` (default `both`).
    pub kind: Option<String>,
    pub r#type: Option<String>,
    pub template: Option<String>,
    pub decimal_separator: Option<String>,
    pub thousand_separator: Option<String>,
    #[serde(default)]
    pub sources: Vec<DisplaySource>,
    pub attribute_class: Option<AttributeClass>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayLabel {
    pub key: Option<String>,
    pub kind: Option<String>,
    pub r#type: Option<String>,
    pub template: Option<String>,
    pub decimal_separator: Option<String>,
    pub thousand_separator: Option<String>,
    #[serde(default)]
    pub sources: Vec<DisplaySource>,
    pub override_existing: Option<bool>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExtraCfg {
    pub entity_auto_color: Option<bool>,
    pub link_match_entity_color: Option<bool>,
    /// `radial` (default) / `circle` / `grid` / `random` / `fr` / `forceatlas2` / `tree`.
    pub arrange: Option<String>,
    pub layout_scale: Option<f64>,
    pub link_arc_offset: Option<i64>,
    pub geo_map: Option<GeoMapCfg>,
    pub icon_map: Option<IconMapCfg>,
    pub styling: Option<StylingCfg>,
    #[serde(default)]
    pub display_attribute: Vec<DisplayAttribute>,
    #[serde(default)]
    pub display_label: Vec<DisplayLabel>,
    /// `referenced` (default) or `all`.
    pub custom_icons_include: Option<String>,
}

/// Chart-wide settings; the `settings` block of a config.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub chart: ChartCfg,
    pub font: Font,
    pub view: ViewCfg,
    pub grid: GridCfg,
    pub wiring: WiringCfg,
    pub links_cfg: LinksCfg,
    pub time: TimeCfg,
    pub summary: SummaryCfg,
    pub legend_cfg: LegendCfg,
    pub extra_cfg: ExtraCfg,
}

// ── Metadata & shared values ────────────────────────────────────────────────

/// ANB timezone: internal `id` (UniqueID 1-122) plus cosmetic `name`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeZone {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CustomProperty {
    pub name: String,
    pub value: String,
}

/// A custom icon definition: either a raw `image` source (path / `data:` URI) to
/// convert, or a pre-baked `data` (base64 of zlib-compressed BMP) + `datalength`.
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomIconEntry {
    pub name: String,
    pub image: Option<String>,
    pub data: Option<String>,
    pub datalength: Option<u32>,
    pub prefix: Option<String>,
}

/// An evidence card attached to an entity or link.
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Card {
    pub summary: Option<String>,
    pub date: Option<String>,
    pub time: Option<String>,
    pub description: Option<String>,
    pub source_ref: Option<String>,
    pub source_type: Option<String>,
    pub grade_one: Option<GradeRef>,
    pub grade_two: Option<GradeRef>,
    pub grade_three: Option<GradeRef>,
    pub datetime_description: Option<String>,
    pub timezone: Option<TimeZone>,
    /// INTERNAL — routes a loose card to an entity; not written to XML.
    pub entity_id: Option<String>,
    /// INTERNAL — routes a loose card to a link; not written to XML.
    pub link_id: Option<String>,
}

// ── Value enforcement (1.17.0) ──────────────────────────────────────────────

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AttributeClassEnforce {
    pub pattern: Option<String>,
    pub description: Option<String>,
    pub allowed_values: Option<Vec<AttrValue>>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EntityTypeEnforce {
    pub id_pattern: Option<String>,
    pub id_pattern_description: Option<String>,
    #[serde(default)]
    pub required_attributes: Vec<String>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LinkTypeEnforce {
    pub id_pattern: Option<String>,
    pub id_pattern_description: Option<String>,
    #[serde(default)]
    pub required_attributes: Vec<String>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Validator {
    pub entity_type: Option<String>,
    pub link_type: Option<String>,
    pub attribute: Option<String>,
    pub pattern: Option<String>,
    pub allowed_values: Option<Vec<AttrValue>>,
    pub description: Option<String>,
}

// ── Attribute & type definitions ────────────────────────────────────────────

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AttributeClass {
    pub name: String,
    pub r#type: Option<AttributeType>,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub decimal_places: Option<i64>,
    pub show_value: Option<bool>,
    pub show_date: Option<bool>,
    pub show_time: Option<bool>,
    pub show_seconds: Option<bool>,
    pub show_if_set: Option<bool>,
    pub show_class_name: Option<bool>,
    pub show_symbol: Option<bool>,
    pub visible: Option<bool>,
    pub is_user: Option<bool>,
    pub user_can_add: Option<bool>,
    pub user_can_remove: Option<bool>,
    pub icon_file: Option<String>,
    pub semantic_type: Option<String>,
    pub merge_behaviour: Option<MergeBehaviour>,
    pub paste_behaviour: Option<MergeBehaviour>,
    #[serde(default)]
    pub font: Font,
    pub enforce: Option<AttributeClassEnforce>,
}

/// A link between two entities.
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Link {
    pub from_id: String,
    pub to_id: String,
    pub r#type: Option<String>,
    pub arrow: Option<String>,
    pub label: Option<String>,
    pub date: Option<String>,
    pub time: Option<String>,
    pub description: Option<String>,
    pub strength: Option<String>,
    pub line_color: Option<ColorValue>,
    pub line_width: Option<i64>,
    pub offset: Option<i64>,
    pub ordered: Option<bool>,
    pub grade_one: Option<GradeRef>,
    pub grade_two: Option<GradeRef>,
    pub grade_three: Option<GradeRef>,
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
    /// `multiple` / `single` / `directed`, or an ANB token (kept as-is).
    pub multiplicity: Option<String>,
    pub fan_out: Option<i64>,
    /// ThemeWiring value or ANB token.
    pub theme_wiring: Option<String>,
    /// INTERNAL — target for loose-card attachment; not written to XML.
    pub link_id: Option<String>,
    pub semantic_type: Option<String>,
}

/// A line dash/dot strength.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Strength {
    pub name: String,
    pub dot_style: DotStyle,
}

impl Default for Strength {
    fn default() -> Self {
        Self {
            name: String::new(),
            dot_style: DotStyle::Solid,
        }
    }
}

/// A grade dimension: ordered names plus an optional default.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GradeCollection {
    pub default: Option<String>,
    #[serde(default)]
    pub items: Vec<String>,
}

/// The strength dimension: ordered strengths plus an optional default.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StrengthCollection {
    pub default: Option<String>,
    #[serde(default)]
    pub items: Vec<Strength>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EntityType {
    pub name: String,
    pub icon_file: Option<String>,
    pub color: Option<ColorValue>,
    pub shade_color: Option<ColorValue>,
    /// Representation value (`icon`, `box`, ...) or ANB token.
    pub representation: Option<String>,
    pub semantic_type: Option<String>,
    pub enforce: Option<EntityTypeEnforce>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LinkType {
    pub name: String,
    pub color: Option<ColorValue>,
    pub semantic_type: Option<String>,
    pub enforce: Option<LinkTypeEnforce>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PaletteAttributeEntry {
    pub name: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Palette {
    pub name: String,
    pub locked: bool,
    #[serde(default)]
    pub entity_types: Vec<String>,
    #[serde(default)]
    pub link_types: Vec<String>,
    #[serde(default)]
    pub attribute_classes: Vec<String>,
    #[serde(default)]
    pub attribute_entries: Vec<PaletteAttributeEntry>,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            name: "Standard".to_string(),
            locked: false,
            entity_types: Vec::new(),
            link_types: Vec::new(),
            attribute_classes: Vec::new(),
            attribute_entries: Vec::new(),
        }
    }
}

#[skip_serializing_none]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LegendItem {
    /// Identity field; config files may spell it `label` (aliased here).
    #[serde(alias = "label")]
    pub name: String,
    /// Legend item type (`font` default, `text`, `icon`, ...) or ANB token.
    pub item_type: Option<String>,
    pub color: Option<ColorValue>,
    pub line_width: Option<i64>,
    pub dash_style: Option<String>,
    pub arrows: Option<String>,
    pub image_name: Option<String>,
    pub shade_color: Option<ColorValue>,
    #[serde(default)]
    pub font: Font,
}

impl Default for LegendItem {
    fn default() -> Self {
        Self {
            name: String::new(),
            item_type: Some("Font".to_string()),
            color: None,
            line_width: None,
            dash_style: None,
            arrows: None,
            image_name: None,
            shade_color: None,
            font: Font::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DateTimeFormat {
    pub name: String,
    pub format: String,
}

// ── Semantic types ──────────────────────────────────────────────────────────

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SemanticEntity {
    pub name: String,
    pub kind_of: String,
    pub guid: Option<String>,
    #[serde(rename = "abstract")]
    pub abstract_: bool,
    pub synonyms: Option<Vec<String>>,
    pub description: Option<String>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SemanticLink {
    pub name: String,
    pub kind_of: String,
    pub guid: Option<String>,
    #[serde(rename = "abstract")]
    pub abstract_: bool,
    pub synonyms: Option<Vec<String>>,
    pub description: Option<String>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SemanticProperty {
    pub name: String,
    pub base_property: String,
    pub guid: Option<String>,
    #[serde(rename = "abstract")]
    pub abstract_: bool,
    pub synonyms: Option<Vec<String>>,
    pub description: Option<String>,
}
