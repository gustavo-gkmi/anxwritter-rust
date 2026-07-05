//! Typed intermediate between input models and XML emission, mirroring the role
//! of `anxwritter/resolved.py`.
//!
//! After resolution, ids are minted, attribute classes are registered, colours
//! are integers, and layout positions are fixed — so the emit pass is a
//! straight-line walk with no further lookups.

/// A resolved evidence card, with datetime parsed and grades resolved.
#[derive(Debug, Clone, Default)]
pub struct ResolvedCard {
    pub summary: Option<String>,
    pub datetime: Option<String>,
    pub date_set: bool,
    pub time_set: bool,
    pub grade_one: Option<i64>,
    pub grade_two: Option<i64>,
    pub grade_three: Option<i64>,
    pub source_ref: Option<String>,
    pub source_type: Option<String>,
    pub description: Option<String>,
    pub datetime_description: Option<String>,
    pub timezone_id: Option<i64>,
    pub timezone_name: Option<String>,
}

/// A resolved attribute: its class name, the minted class reference id, and the
/// formatted value (already stringified per its inferred type).
#[derive(Debug, Clone)]
pub struct ResolvedAttr {
    pub class: String,
    pub reference: String,
    pub value: Option<String>,
}

/// A resolved entity ready to emit.
#[derive(Debug, Clone)]
pub struct ResolvedEntity {
    pub ci_id: String,
    pub int_id: i64,
    pub identity: String,
    pub label: String,
    pub etype: String,
    /// Reference id of the entity type, if it was registered as a collection
    /// entry (only configured/metadata-bearing types are).
    pub etype_ref: Option<String>,
    pub x: i64,
    pub y: i64,
    pub attrs: Vec<ResolvedAttr>,
    /// Resolved grade indices (after name/default resolution).
    pub grade_one: Option<i64>,
    pub grade_two: Option<i64>,
    pub grade_three: Option<i64>,
    /// Auto-assigned shade colour (Icon/ThemeLine/EventFrame), if auto-colour
    /// applied and no explicit colour was set.
    pub auto_shade: Option<u32>,
    /// Label background / foreground colours for the `<CIStyle>` font.
    pub label_bg: Option<u32>,
    pub label_fg: Option<u32>,
    pub cards: Vec<ResolvedCard>,
    /// Per-instance semantic-type GUID (resolved from a name or passed through).
    pub semantic_guid: Option<String>,
    /// Icon override (explicit `icon` field or icon-map result) →
    /// OverrideTypeIcon/TypeIconName.
    pub icon_override: Option<String>,
}

// Links are resolved lazily during emit (see `builder::LinkMeta` /
// `emit_link_item`), so no fully materialized resolved-link type is needed.
