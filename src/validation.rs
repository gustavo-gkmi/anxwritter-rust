//! Chart validation, mirroring `anxwritter/validation.py`.
//!
//! Validators accumulate a `Vec<ValidationError>` rather than failing fast, so
//! the caller sees every problem at once. This covers the high-value rule groups
//! a server needs to reject malformed input: required/unique ids, dangling link
//! endpoints, self-loops, colour validity, enum membership, and grade ranges.

use std::collections::HashMap;
use std::collections::HashSet;

use crate::color::ColorValue;
use crate::datetime::{parse_date, parse_time};
use crate::entities::Entity;
use crate::error::{ErrorType, ValidationError};
use crate::input::{ChartData, Config};
use crate::models::GradeRef;
use crate::value::AttrValue;

const VALID_ARROWS: &[&str] = &[
    "ArrowOnHead",
    "ArrowOnTail",
    "ArrowOnBoth",
    "head",
    "tail",
    "both",
    "->",
    "<-",
    "<->",
];
const VALID_MULTIPLICITY: &[&str] = &[
    "MultiplicityMultiple",
    "MultiplicitySingle",
    "MultiplicityDirected",
    "multiple",
    "single",
    "directed",
];
const VALID_THEME_WIRING: &[&str] = &[
    "KeepsAtEventHeight",
    "ReturnsToThemeHeight",
    "GoesToNextEventHeight",
    "NoDiversion",
    "keep_event",
    "return_theme",
    "next_event",
    "no_diversion",
];
const VALID_LEGEND_TYPES: &[&str] = &[
    "Font",
    "Text",
    "Icon",
    "Attribute",
    "Line",
    "Link",
    "TimeZone",
    "IconFrame",
    "font",
    "text",
    "icon",
    "attribute",
    "line",
    "link",
    "timezone",
    "icon_frame",
];

fn err(t: ErrorType, loc: impl Into<String>, msg: impl Into<String>) -> ValidationError {
    ValidationError::new(t, loc, msg)
}

fn color_valid(c: &ColorValue) -> bool {
    match c {
        ColorValue::Int(v) => *v <= 0x00FF_FFFF,
        _ => c.to_colorref().is_ok(),
    }
}

fn check_color(out: &mut Vec<ValidationError>, c: &Option<ColorValue>, field: &str, loc: &str) {
    if let Some(c) = c {
        if !color_valid(c) {
            out.push(err(
                ErrorType::UnknownColor,
                loc.to_string(),
                format!("Invalid color in '{field}'"),
            ));
        }
    }
}

/// The colour fields a given entity representation can carry.
fn entity_colors(e: &Entity) -> Vec<(&'static str, &ColorValue)> {
    let opts: [(&'static str, &Option<ColorValue>); 3] = match e {
        Entity::Icon(i) => [("color", &i.color), ("color", &None), ("color", &None)],
        Entity::Box(b) => [
            ("bg_color", &b.bg_color),
            ("line_color", &b.line_color),
            ("", &None),
        ],
        Entity::Circle(c) => [("bg_color", &c.bg_color), ("", &None), ("", &None)],
        Entity::ThemeLine(t) => [
            ("shade_color", &t.shade_color),
            ("line_color", &t.line_color),
            ("", &None),
        ],
        Entity::EventFrame(ev) => [
            ("shade_color", &ev.shade_color),
            ("bg_color", &ev.bg_color),
            ("line_color", &ev.line_color),
        ],
        Entity::TextBlock(tb) => [
            ("bg_color", &tb.bg_color),
            ("line_color", &tb.line_color),
            ("", &None),
        ],
        Entity::Label(l) => [
            ("bg_color", &l.bg_color),
            ("line_color", &l.line_color),
            ("", &None),
        ],
    };
    opts.into_iter()
        .filter_map(|(n, c)| c.as_ref().map(|c| (n, c)))
        .collect()
}

fn variant_name(e: &Entity) -> &'static str {
    match e {
        Entity::Icon(_) => "Icon",
        Entity::Box(_) => "Box",
        Entity::Circle(_) => "Circle",
        Entity::ThemeLine(_) => "ThemeLine",
        Entity::EventFrame(_) => "EventFrame",
        Entity::TextBlock(_) => "TextBlock",
        Entity::Label(_) => "Label",
    }
}

fn check_grade(
    out: &mut Vec<ValidationError>,
    g: &Option<GradeRef>,
    items: &[String],
    field: &str,
    loc: &str,
) {
    match g {
        None => {}
        Some(GradeRef::Index(i)) => {
            if *i < 0 || *i as usize >= items.len() {
                out.push(err(
                    ErrorType::GradeOutOfRange,
                    loc.to_string(),
                    format!("{field} index {i} out of range (0..{})", items.len()),
                ));
            }
        }
        // A digit string is treated as an index elsewhere; only flag real names.
        Some(GradeRef::Name(n)) if n.parse::<i64>().is_err() && !items.iter().any(|x| x == n) => {
            out.push(err(
                ErrorType::UnknownGrade,
                loc.to_string(),
                format!("Unknown {field} '{n}'"),
            ));
        }
        Some(GradeRef::Name(_)) => {}
    }
}

/// Validate a whole chart (one merged config layer + data). Returns every error.
pub fn validate(config: &Config, data: &ChartData) -> Vec<ValidationError> {
    let mut out = Vec::new();

    // Effective strength names: configured set, or the implicit `Default` the
    // chart always carries (matches Python's `chart.strengths == ['Default']`).
    let strength_names: HashSet<&str> = match &config.strengths {
        Some(s) => s.items.iter().map(|x| x.name.as_str()).collect(),
        None => HashSet::from(["Default"]),
    };
    // Registered datetime-format names (an item referencing an unregistered one
    // is `unregistered_datetime_format`).
    let dtf_names: HashSet<&str> = config
        .datetime_formats
        .iter()
        .map(|f| f.name.as_str())
        .collect();
    let g1: Vec<String> = config
        .grades_one
        .as_ref()
        .map(|g| g.items.clone())
        .unwrap_or_default();
    let g2: Vec<String> = config
        .grades_two
        .as_ref()
        .map(|g| g.items.clone())
        .unwrap_or_default();
    let g3: Vec<String> = config
        .grades_three
        .as_ref()
        .map(|g| g.items.clone())
        .unwrap_or_default();

    validate_type_defs(&mut out, config);

    // ── Entities ──
    let entities = collect_entities(data);
    let mut entity_ids: HashMap<String, String> = HashMap::new();
    let mut attr_types: HashMap<String, &'static str> = HashMap::new();
    for (i, e) in entities.iter().enumerate() {
        let loc = format!("entities[{i}] ({})", variant_name(e));
        let c = e.common();
        if c.id.is_empty() {
            out.push(err(
                ErrorType::MissingRequired,
                &loc,
                "Missing required field 'id'",
            ));
            continue;
        }
        if c.r#type.is_empty() {
            out.push(err(
                ErrorType::MissingRequired,
                &loc,
                format!("Missing required field 'type' on entity '{}'", c.id),
            ));
        }
        if let Some(first) = entity_ids.get(&c.id) {
            out.push(err(
                ErrorType::DuplicateId,
                &loc,
                format!("Duplicate entity id '{}' (first seen at {first})", c.id),
            ));
        } else {
            entity_ids.insert(c.id.clone(), loc.clone());
        }
        for (field, col) in entity_colors(e) {
            check_color(&mut out, &Some(col.clone()), field, &loc);
        }
        check_color(&mut out, &c.label_font.color, "label_font.color", &loc);
        check_chart_item_common(
            &mut out,
            &loc,
            c.date.as_deref(),
            c.time.as_deref(),
            c.strength.as_deref(),
            &strength_names,
            &c.grade_one,
            &c.grade_two,
            &c.grade_three,
            &g1,
            &g2,
            &g3,
            c.datetime_format.as_deref(),
            &dtf_names,
        );
        // Attribute type consistency across the chart.
        for (name, val) in &c.attributes {
            let t = val.infer_type().anb_token();
            if let Some(prev) = attr_types.get(name) {
                if *prev != t {
                    out.push(err(
                        ErrorType::TypeConflict,
                        &loc,
                        format!("Attribute '{name}' used as {prev} and {t}"),
                    ));
                }
            } else {
                attr_types.insert(name.clone(), t);
            }
        }
    }

    // Entity representation per id, for the `ordered` link rule.
    let entity_class: HashMap<&str, &'static str> = entities
        .iter()
        .map(|e| (e.common().id.as_str(), variant_name(e)))
        .collect();

    // ── Links ──
    for (i, l) in data.links.iter().enumerate() {
        let loc = format!("links[{i}]");
        if l.from_id.is_empty() {
            out.push(err(
                ErrorType::MissingRequired,
                &loc,
                "Missing required field 'from_id'",
            ));
        } else if !entity_ids.contains_key(&l.from_id) {
            out.push(err(
                ErrorType::MissingEntity,
                &loc,
                format!(
                    "Link from_id '{}' does not reference a known entity",
                    l.from_id
                ),
            ));
        }
        if l.to_id.is_empty() {
            out.push(err(
                ErrorType::MissingRequired,
                &loc,
                "Missing required field 'to_id'",
            ));
        } else if !entity_ids.contains_key(&l.to_id) {
            out.push(err(
                ErrorType::MissingEntity,
                &loc,
                format!("Entity '{}' (to_id) not found in any entity list", l.to_id),
            ));
        }
        if !l.from_id.is_empty() && l.from_id == l.to_id {
            out.push(err(
                ErrorType::SelfLoop,
                &loc,
                format!("Link forms a self-loop on '{}'", l.from_id),
            ));
        }
        check_color(&mut out, &l.line_color, "line_color", &loc);
        check_enum(
            &mut out,
            &l.arrow,
            VALID_ARROWS,
            ErrorType::InvalidArrow,
            "arrow",
            &loc,
        );
        check_enum(
            &mut out,
            &l.multiplicity,
            VALID_MULTIPLICITY,
            ErrorType::InvalidMultiplicity,
            "multiplicity",
            &loc,
        );
        check_enum(
            &mut out,
            &l.theme_wiring,
            VALID_THEME_WIRING,
            ErrorType::InvalidThemeWiring,
            "theme_wiring",
            &loc,
        );
        check_chart_item_common(
            &mut out,
            &loc,
            l.date.as_deref(),
            l.time.as_deref(),
            l.strength.as_deref(),
            &strength_names,
            &l.grade_one,
            &l.grade_two,
            &l.grade_three,
            &g1,
            &g2,
            &g3,
            l.datetime_format.as_deref(),
            &dtf_names,
        );
        // `ordered` requires both endpoints to be ThemeLine entities.
        if l.ordered == Some(true) {
            let is_tl = |id: &str| entity_class.get(id) == Some(&"ThemeLine");
            if !is_tl(&l.from_id) || !is_tl(&l.to_id) {
                out.push(err(
                    ErrorType::InvalidOrdered,
                    &loc,
                    "ordered=True requires both ends to be ThemeLine entities",
                ));
            }
        }
    }

    // ── Legend items ──
    for (i, li) in config.legend_items.iter().enumerate() {
        let loc = format!("legend_items[{i}]");
        if li.name.is_empty() {
            out.push(err(
                ErrorType::MissingRequired,
                &loc,
                "Legend item missing 'name'/'label'",
            ));
        }
        if let Some(t) = &li.item_type {
            if !VALID_LEGEND_TYPES.contains(&t.as_str()) {
                out.push(err(
                    ErrorType::InvalidLegendType,
                    &loc,
                    format!("Invalid legend item_type '{t}'"),
                ));
            }
        }
    }

    // ── Strength default ──
    if let Some(sc) = &config.strengths {
        if let Some(d) = &sc.default {
            if !sc.items.iter().any(|s| &s.name == d) {
                out.push(err(
                    ErrorType::InvalidStrengthDefault,
                    "strengths",
                    format!("Default strength '{d}' is not among the defined strengths"),
                ));
            }
        }
    }

    validate_attribute_class_behaviours(&mut out, config);
    validate_datetime_formats(&mut out, config);
    validate_semantic_types(&mut out, config);
    validate_geo_map(&mut out, config);
    validate_icon_map(&mut out, config, &entities);
    validate_styling(&mut out, config, data);
    validate_palettes(&mut out, config);
    validate_value_enforcement(&mut out, config, &entities, &data.links);
    validate_validators_config(&mut out, config);
    validate_custom_icons_include(&mut out, config);
    validate_loose_cards(&mut out, data, &entity_ids);
    validate_connection_conflicts(&mut out, &data.links);
    validate_display(&mut out, config);
    validate_enforce_descriptions(&mut out, config);

    out
}

fn merge_valid(ty: crate::enums::AttributeType) -> &'static [&'static str] {
    use crate::enums::AttributeType::*;
    match ty {
        Text => &["add", "add_space", "add_line_break"],
        Number => &["add", "max", "min"],
        Datetime => &["min", "max"],
        Flag => &["or", "and", "xor"],
    }
}
fn paste_valid(ty: crate::enums::AttributeType) -> &'static [&'static str] {
    use crate::enums::AttributeType::*;
    match ty {
        Text => &["add", "add_space", "add_line_break", "assign", "noop"],
        Number => &[
            "add",
            "max",
            "min",
            "subtract",
            "subtract_swap",
            "assign",
            "noop",
        ],
        Datetime => &["min", "max", "assign", "noop"],
        Flag => &["or", "and", "xor", "assign", "noop"],
    }
}

fn serde_str<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|x| x.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Merge/paste behaviour must be valid for the attribute class's type.
fn validate_attribute_class_behaviours(out: &mut Vec<ValidationError>, config: &Config) {
    for (i, ac) in config.attribute_classes.iter().enumerate() {
        let loc = format!("attribute_classes[{i}]");
        let Some(ty) = ac.r#type else { continue };
        if let Some(m) = &ac.merge_behaviour {
            let s = serde_str(m);
            if !merge_valid(ty).contains(&s.as_str()) {
                out.push(err(
                    ErrorType::InvalidMergeBehaviour,
                    &loc,
                    format!(
                        "merge_behaviour '{s}' is not valid for type '{}'",
                        serde_str(&ty)
                    ),
                ));
            }
        }
        if let Some(p) = &ac.paste_behaviour {
            let s = serde_str(p);
            if !paste_valid(ty).contains(&s.as_str()) {
                out.push(err(
                    ErrorType::InvalidPasteBehaviour,
                    &loc,
                    format!(
                        "paste_behaviour '{s}' is not valid for type '{}'",
                        serde_str(&ty)
                    ),
                ));
            }
        }
    }
}

/// Datetime format name required/unique/≤250 chars; format ≤259 chars.
fn validate_datetime_formats(out: &mut Vec<ValidationError>, config: &Config) {
    let mut seen = HashSet::new();
    for (i, f) in config.datetime_formats.iter().enumerate() {
        let loc = format!("datetime_formats[{i}]");
        if f.name.is_empty() {
            out.push(err(
                ErrorType::MissingRequired,
                &loc,
                "DateTimeFormat missing 'name'",
            ));
        } else {
            if f.name.chars().count() > 250 {
                out.push(err(
                    ErrorType::InvalidValue,
                    &loc,
                    "DateTimeFormat name exceeds 250 chars",
                ));
            }
            if !seen.insert(f.name.clone()) {
                out.push(err(
                    ErrorType::DuplicateName,
                    &loc,
                    format!("Duplicate datetime format '{}'", f.name),
                ));
            }
        }
        if f.format.chars().count() > 259 {
            out.push(err(
                ErrorType::InvalidValue,
                &loc,
                "DateTimeFormat format exceeds 259 chars",
            ));
        }
    }
}

/// Semantic type definitions need a name and either kind_of/base_property or
/// abstract; references must be registered or a `guid…` literal; reject LN* COM
/// names.
fn validate_semantic_types(out: &mut Vec<ValidationError>, config: &Config) {
    let ln = regex::Regex::new(r"^LN(Entity|Link|Property)[A-Z]").unwrap();
    for (i, se) in config.semantic_entities.iter().enumerate() {
        let loc = format!("semantic_entities[{i}]");
        if se.name.is_empty() {
            out.push(err(
                ErrorType::MissingRequired,
                &loc,
                "SemanticEntity missing 'name'",
            ));
        }
        if se.kind_of.is_empty() && !se.abstract_ {
            out.push(err(
                ErrorType::MissingRequired,
                &loc,
                "SemanticEntity needs 'kind_of' or abstract",
            ));
        }
    }
    for (i, sl) in config.semantic_links.iter().enumerate() {
        let loc = format!("semantic_links[{i}]");
        if sl.name.is_empty() {
            out.push(err(
                ErrorType::MissingRequired,
                &loc,
                "SemanticLink missing 'name'",
            ));
        }
        if sl.kind_of.is_empty() && !sl.abstract_ {
            out.push(err(
                ErrorType::MissingRequired,
                &loc,
                "SemanticLink needs 'kind_of' or abstract",
            ));
        }
    }
    for (i, sp) in config.semantic_properties.iter().enumerate() {
        let loc = format!("semantic_properties[{i}]");
        if sp.name.is_empty() {
            out.push(err(
                ErrorType::MissingRequired,
                &loc,
                "SemanticProperty missing 'name'",
            ));
        }
        if sp.base_property.is_empty() && !sp.abstract_ {
            out.push(err(
                ErrorType::MissingRequired,
                &loc,
                "SemanticProperty needs 'base_property' or abstract",
            ));
        }
    }
    let ent: HashSet<&str> = config
        .semantic_entities
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    let lnk: HashSet<&str> = config
        .semantic_links
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    let prop: HashSet<&str> = config
        .semantic_properties
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    let mut check = |val: &Option<String>, loc: &str, is_prop: bool| {
        if let Some(s) = val.as_deref().filter(|s| !s.is_empty()) {
            if ln.is_match(s) {
                out.push(err(
                    ErrorType::InvalidSemanticType,
                    loc,
                    format!("'{s}' uses a reserved i2 COM name"),
                ));
            } else if !s.starts_with("guid") {
                let known = if is_prop {
                    prop.contains(s)
                } else {
                    ent.contains(s) || lnk.contains(s)
                };
                if !known {
                    out.push(err(
                        ErrorType::UnknownSemanticType,
                        loc,
                        format!("Unknown semantic type '{s}'"),
                    ));
                }
            }
        }
    };
    for (i, et) in config.entity_types.iter().enumerate() {
        check(
            &et.semantic_type,
            &format!("entity_types[{i}].semantic_type"),
            false,
        );
    }
    for (i, lt) in config.link_types.iter().enumerate() {
        check(
            &lt.semantic_type,
            &format!("link_types[{i}].semantic_type"),
            false,
        );
    }
    for (i, ac) in config.attribute_classes.iter().enumerate() {
        check(
            &ac.semantic_type,
            &format!("attribute_classes[{i}].semantic_type"),
            true,
        );
    }
}

fn validate_geo_map(out: &mut Vec<ValidationError>, config: &Config) {
    let Some(g) = config
        .settings
        .as_ref()
        .and_then(|s| s.extra_cfg.geo_map.as_ref())
    else {
        return;
    };
    let loc = "settings.extra_cfg.geo_map";
    if g.attribute_name.as_deref().unwrap_or("").is_empty() {
        out.push(err(
            ErrorType::InvalidGeoMap,
            loc,
            "geo_map requires 'attribute_name'",
        ));
    }
    if let Some(m) = &g.mode {
        if !["position", "latlon", "both"].contains(&m.as_str()) {
            out.push(err(
                ErrorType::InvalidGeoMap,
                loc,
                format!("invalid geo_map mode '{m}'"),
            ));
        }
    }
    if g.width.is_some_and(|w| w <= 0) || g.height.is_some_and(|h| h <= 0) {
        out.push(err(
            ErrorType::InvalidGeoMap,
            loc,
            "geo_map width/height must be positive",
        ));
    }
    if g.spread_radius.is_some_and(|r| r < 0) {
        out.push(err(
            ErrorType::InvalidGeoMap,
            loc,
            "geo_map spread_radius must be >= 0",
        ));
    }
    if g.data.is_none() && g.data_file.is_none() {
        out.push(err(
            ErrorType::InvalidGeoMap,
            loc,
            "geo_map needs 'data' or 'data_file'",
        ));
    }
    if let Some(data) = &g.data {
        for (k, v) in data {
            if v.len() != 2 {
                out.push(err(
                    ErrorType::InvalidGeoMap,
                    loc,
                    format!("geo_map data['{k}'] must be [lat, lon]"),
                ));
            } else {
                if !(-90.0..=90.0).contains(&v[0]) {
                    out.push(err(
                        ErrorType::InvalidGeoMap,
                        loc,
                        format!("latitude out of range for '{k}'"),
                    ));
                }
                if !(-180.0..=180.0).contains(&v[1]) {
                    out.push(err(
                        ErrorType::InvalidGeoMap,
                        loc,
                        format!("longitude out of range for '{k}'"),
                    ));
                }
            }
        }
    }
}

fn validate_icon_map(out: &mut Vec<ValidationError>, config: &Config, entities: &[Entity]) {
    let Some(im) = config
        .settings
        .as_ref()
        .and_then(|s| s.extra_cfg.icon_map.as_ref())
    else {
        return;
    };
    let mut known: HashSet<String> = config.entity_types.iter().map(|e| e.name.clone()).collect();
    for e in entities {
        known.insert(e.common().r#type.clone());
    }
    for (i, r) in im.rules.iter().enumerate() {
        let loc = format!("settings.extra_cfg.icon_map.rules[{i}]");
        let m = r.r#match.as_deref().unwrap_or("attribute");
        if m != "attribute" && m != "id" {
            out.push(err(
                ErrorType::IconMapInvalid,
                &loc,
                format!("invalid match '{m}'"),
            ));
        }
        if r.mapping.is_empty() {
            out.push(err(
                ErrorType::IconMapInvalid,
                &loc,
                "icon rule has empty mapping",
            ));
        }
        if m == "id"
            && (r.default.is_some() || r.default_when_absent.is_some() || r.r#type.is_some())
        {
            out.push(err(
                ErrorType::IconMapInvalid,
                &loc,
                "id rule cannot have default/default_when_absent/type",
            ));
        }
        if m == "attribute" && r.attribute_name.as_deref().unwrap_or("").is_empty() {
            out.push(err(
                ErrorType::IconMapInvalid,
                &loc,
                "attribute rule needs 'attribute_name'",
            ));
        }
        if let Some(t) = &r.r#type {
            if !known.contains(t) {
                out.push(err(
                    ErrorType::IconMapInvalid,
                    &loc,
                    format!("unknown type filter '{t}'"),
                ));
            }
        }
    }
}

fn validate_styling(out: &mut Vec<ValidationError>, config: &Config, data: &ChartData) {
    let Some(links) = config
        .settings
        .as_ref()
        .and_then(|s| s.extra_cfg.styling.as_ref())
        .and_then(|s| s.links.as_ref())
    else {
        return;
    };
    let base = "settings.extra_cfg.styling.links";
    if let Some(ic) = &links.intensity {
        let loc = format!("{base}.intensity");
        if ic.width.is_none() && ic.color.is_none() {
            // nothing to do
        }
        if let Some(w) = &ic.width {
            match &w.range {
                None => out.push(err(
                    ErrorType::InvalidIntensityRange,
                    &loc,
                    "width.range is required",
                )),
                Some(r) if r.len() != 2 => {
                    out.push(err(
                        ErrorType::InvalidIntensityRange,
                        &loc,
                        "width.range must be [min, max]",
                    ));
                }
                Some(r) if r[0] < 0 || r[1] < 0 => {
                    out.push(err(
                        ErrorType::InvalidIntensityRange,
                        &loc,
                        "width.range must be >= 0",
                    ));
                }
                Some(r) if r[0] >= r[1] => {
                    out.push(err(
                        ErrorType::InvalidIntensityRange,
                        &loc,
                        "width.range min must be < max",
                    ));
                }
                _ => {}
            }
        }
        if let Some(c) = &ic.color {
            match &c.ramp {
                None => out.push(err(
                    ErrorType::InvalidIntensityRamp,
                    &loc,
                    "color.ramp is required",
                )),
                Some(r) if r.len() < 2 => {
                    out.push(err(
                        ErrorType::InvalidIntensityRamp,
                        &loc,
                        "color.ramp needs >= 2 colors",
                    ));
                }
                _ => {}
            }
            if let Some(sp) = &c.space {
                if !["rgb", "rgb_linear", "hsl"].contains(&sp.as_str()) {
                    out.push(err(
                        ErrorType::InvalidIntensityConfig,
                        &loc,
                        format!("invalid color space '{sp}'"),
                    ));
                }
            }
        }
        let attr = ic.attribute.as_deref().unwrap_or("");
        let w_attr = ic
            .width
            .as_ref()
            .and_then(|w| w.attribute.as_deref())
            .unwrap_or(attr);
        let c_attr = ic
            .color
            .as_ref()
            .and_then(|c| c.attribute.as_deref())
            .unwrap_or(attr);
        if (ic.width.is_some() && w_attr.is_empty()) || (ic.color.is_some() && c_attr.is_empty()) {
            out.push(err(
                ErrorType::InvalidIntensityAttribute,
                &loc,
                "intensity requires an 'attribute'",
            ));
        }
        // Per-sub scale checks: `power` needs a value > 0; `log` needs every
        // value on the attribute > 0 (mirrors `_validate_intensity_block`).
        let top_scale = ic.scale.as_deref();
        if let Some(w) = &ic.width {
            check_intensity_sub(
                out,
                &loc,
                w.scale.as_deref().or(top_scale),
                w.power,
                w_attr,
                data,
            );
        }
        if let Some(c) = &ic.color {
            check_intensity_sub(
                out,
                &loc,
                c.scale.as_deref().or(top_scale),
                c.power,
                c_attr,
                data,
            );
        }
    }
    if let Some(cc) = &links.categorical {
        let loc = format!("{base}.categorical");
        if cc.attribute.as_deref().unwrap_or("").is_empty() {
            out.push(err(
                ErrorType::InvalidCategoricalAttribute,
                &loc,
                "categorical requires 'attribute'",
            ));
        }
        if cc.styles.is_empty() {
            out.push(err(
                ErrorType::InvalidCategoricalConfig,
                &loc,
                "categorical requires non-empty 'styles'",
            ));
        }
        // Each style entry must set at least one field.
        for (k, st) in &cc.styles {
            if st.line_color.is_none() && st.line_width.is_none() && st.strength.is_none() {
                out.push(err(
                    ErrorType::InvalidCategoricalStyle,
                    &loc,
                    format!(
                        "categorical style at styles['{k}'] has no settable fields (line_color/line_width/strength)"
                    ),
                ));
            }
        }
    }

    // Conflict: intensity and categorical targeting the same attribute.
    if let (Some(ic), Some(cc)) = (&links.intensity, &links.categorical) {
        let mut i_attrs: HashSet<&str> = HashSet::new();
        let top = ic.attribute.as_deref();
        if let Some(a) = top {
            i_attrs.insert(a);
        }
        if let Some(a) = ic
            .width
            .as_ref()
            .and_then(|w| w.attribute.as_deref())
            .or(top)
        {
            i_attrs.insert(a);
        }
        if let Some(a) = ic
            .color
            .as_ref()
            .and_then(|c| c.attribute.as_deref())
            .or(top)
        {
            i_attrs.insert(a);
        }
        if let Some(ca) = cc.attribute.as_deref() {
            if i_attrs.contains(ca) {
                out.push(err(
                    ErrorType::StylingConflict,
                    base,
                    format!("intensity and categorical both target attribute '{ca}' — pick one."),
                ));
            }
        }
    }
}

/// Numeric value of an attribute (int/float), or `None` for text/flag.
fn numeric_value(v: &AttrValue) -> Option<f64> {
    match v {
        AttrValue::Int(n) => Some(*n as f64),
        AttrValue::Float(f) => Some(*f),
        _ => None,
    }
}

/// Intensity per-sub scale checks: `power` requires a positive `power` value;
/// `log` requires every value of `attr` across links to be > 0.
fn check_intensity_sub(
    out: &mut Vec<ValidationError>,
    loc: &str,
    sub_scale: Option<&str>,
    power: Option<f64>,
    attr: &str,
    data: &ChartData,
) {
    let scale = sub_scale.unwrap_or("sqrt");
    if scale == "power" && power.filter(|p| *p > 0.0).is_none() {
        out.push(err(
            ErrorType::InvalidIntensityConfig,
            loc,
            "intensity.scale='power' requires 'power' > 0",
        ));
    }
    if scale == "log" {
        let non_positive = data
            .links
            .iter()
            .filter_map(|l| l.attributes.get(attr).and_then(numeric_value))
            .any(|v| v <= 0.0);
        if non_positive {
            out.push(err(
                ErrorType::InvalidIntensityDomain,
                loc,
                format!("intensity.scale='log' requires every value > 0 on attribute '{attr}'"),
            ));
        }
    }
}

fn validate_palettes(out: &mut Vec<ValidationError>, config: &Config) {
    let et: HashSet<&str> = config
        .entity_types
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    let lt: HashSet<&str> = config.link_types.iter().map(|e| e.name.as_str()).collect();
    let ac: HashSet<&str> = config
        .attribute_classes
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    for (i, p) in config.palettes.iter().enumerate() {
        let loc = format!("palettes[{i}]");
        for name in &p.entity_types {
            if !et.contains(name.as_str()) {
                out.push(err(
                    ErrorType::PaletteUnknownRef,
                    &loc,
                    format!("unknown entity type '{name}'"),
                ));
            }
        }
        for name in &p.link_types {
            if !lt.contains(name.as_str()) {
                out.push(err(
                    ErrorType::PaletteUnknownRef,
                    &loc,
                    format!("unknown link type '{name}'"),
                ));
            }
        }
        for name in &p.attribute_classes {
            if !ac.contains(name.as_str()) {
                out.push(err(
                    ErrorType::PaletteUnknownRef,
                    &loc,
                    format!("unknown attribute class '{name}'"),
                ));
            }
        }
    }
}

fn compile(pat: &str) -> Option<regex::Regex> {
    regex::Regex::new(&format!("^(?:{pat})$")).ok()
}

fn validate_value_enforcement(
    out: &mut Vec<ValidationError>,
    config: &Config,
    entities: &[Entity],
    links: &[crate::models::Link],
) {
    use std::collections::HashMap;
    // EntityType/LinkType id patterns + required attributes.
    let mut et_id: HashMap<&str, regex::Regex> = HashMap::new();
    let mut et_req: HashMap<&str, &Vec<String>> = HashMap::new();
    for et in &config.entity_types {
        if let Some(en) = &et.enforce {
            if let Some(p) = &en.id_pattern {
                if let Some(re) = compile(p) {
                    et_id.insert(et.name.as_str(), re);
                }
            }
            if !en.required_attributes.is_empty() {
                et_req.insert(et.name.as_str(), &en.required_attributes);
            }
        }
    }
    // AttributeClass value enforcement.
    let mut ac_pat: PatRules = HashMap::new();
    let mut ac_allowed: AllowedRules = HashMap::new();
    for ac in &config.attribute_classes {
        if let Some(en) = &ac.enforce {
            if let Some(p) = &en.pattern {
                if let Some(re) = compile(p) {
                    ac_pat.insert(ac.name.as_str(), (re, en.description.clone()));
                }
            } else if let Some(av) = &en.allowed_values {
                ac_allowed.insert(ac.name.as_str(), av);
            }
        }
    }
    for (i, e) in entities.iter().enumerate() {
        let c = e.common();
        let loc = format!("entities[{i}] ({})", variant_name(e));
        if let Some(re) = et_id.get(c.r#type.as_str()) {
            if !c.id.is_empty() && !re.is_match(&c.id) {
                out.push(err(
                    ErrorType::IdPatternMismatch,
                    &loc,
                    format!("id '{}' does not match the type pattern", c.id),
                ));
            }
        }
        if let Some(req) = et_req.get(c.r#type.as_str()) {
            for a in *req {
                if !c.attributes.contains_key(a) {
                    out.push(err(
                        ErrorType::RequiredAttributeMissing,
                        &loc,
                        format!("missing required attribute '{a}'"),
                    ));
                }
            }
        }
        check_attr_rules(out, &loc, &c.attributes, &ac_pat, &ac_allowed);
    }
    let mut lt_req: HashMap<&str, &Vec<String>> = HashMap::new();
    for lt in &config.link_types {
        if let Some(en) = &lt.enforce {
            if !en.required_attributes.is_empty() {
                lt_req.insert(lt.name.as_str(), &en.required_attributes);
            }
        }
    }
    for (i, l) in links.iter().enumerate() {
        let loc = format!("links[{i}]");
        if let Some(t) = &l.r#type {
            if let Some(req) = lt_req.get(t.as_str()) {
                for a in *req {
                    if !l.attributes.contains_key(a) {
                        out.push(err(
                            ErrorType::RequiredAttributeMissing,
                            &loc,
                            format!("missing required attribute '{a}'"),
                        ));
                    }
                }
            }
        }
        check_attr_rules(out, &loc, &l.attributes, &ac_pat, &ac_allowed);
    }
}

type AttrValueModel = crate::value::AttrValue;
type PatRules<'a> = std::collections::HashMap<&'a str, (regex::Regex, Option<String>)>;
type AllowedRules<'a> = std::collections::HashMap<&'a str, &'a Vec<AttrValueModel>>;

fn check_attr_rules(
    out: &mut Vec<ValidationError>,
    loc: &str,
    attrs: &indexmap::IndexMap<String, AttrValueModel>,
    ac_pat: &PatRules,
    ac_allowed: &AllowedRules,
) {
    for (name, val) in attrs {
        if let Some((re, desc)) = ac_pat.get(name.as_str()) {
            let s = val.render();
            if !re.is_match(&s) {
                let d = desc.clone().unwrap_or_else(|| "pattern mismatch".into());
                out.push(err(
                    ErrorType::AttributePatternMismatch,
                    loc,
                    format!("attribute '{name}' value '{s}': {d}"),
                ));
            }
        }
        if let Some(allowed) = ac_allowed.get(name.as_str()) {
            if !allowed.iter().any(|a| a == val) {
                out.push(err(
                    ErrorType::AttributeValueNotAllowed,
                    loc,
                    format!("attribute '{name}' value not allowed"),
                ));
            }
        }
    }
}

fn validate_validators_config(out: &mut Vec<ValidationError>, config: &Config) {
    let et: HashSet<&str> = config
        .entity_types
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    let lt: HashSet<&str> = config.link_types.iter().map(|e| e.name.as_str()).collect();
    let mut keys = HashSet::new();
    for (i, v) in config.validators.iter().enumerate() {
        let loc = format!("validators[{i}]");
        let has_et = v.entity_type.is_some();
        let has_lt = v.link_type.is_some();
        if has_et == has_lt {
            out.push(err(
                ErrorType::ValidatorInvalidScope,
                &loc,
                "validator needs exactly one of entity_type/link_type",
            ));
        }
        match &v.attribute {
            None => out.push(err(
                ErrorType::MissingRequired,
                &loc,
                "validator missing 'attribute'",
            )),
            Some(a) if a == "id" => {
                out.push(err(
                    ErrorType::ValidatorReservedAttribute,
                    &loc,
                    "'id' is reserved; use enforce.id_pattern",
                ));
            }
            _ => {}
        }
        if v.pattern.is_some() == v.allowed_values.is_some() {
            out.push(err(
                ErrorType::ValidatorInvalidShape,
                &loc,
                "validator needs exactly one of pattern/allowed_values",
            ));
        }
        if v.pattern.is_some() && v.description.is_none() {
            out.push(err(
                ErrorType::PatternMissingDescription,
                &loc,
                "pattern validator needs a 'description'",
            ));
        }
        if let Some(t) = &v.entity_type {
            if !et.contains(t.as_str()) {
                out.push(err(
                    ErrorType::ValidatorUnknownType,
                    &loc,
                    format!("unknown entity type '{t}'"),
                ));
            }
        }
        if let Some(t) = &v.link_type {
            if !lt.contains(t.as_str()) {
                out.push(err(
                    ErrorType::ValidatorUnknownType,
                    &loc,
                    format!("unknown link type '{t}'"),
                ));
            }
        }
        if let (Some(scope), Some(attr)) = (
            v.entity_type.as_ref().or(v.link_type.as_ref()),
            &v.attribute,
        ) {
            if !keys.insert(format!("{scope}::{attr}")) {
                out.push(err(
                    ErrorType::ValidatorDuplicateKey,
                    &loc,
                    "duplicate validator scope/attribute",
                ));
            }
        }
    }
}

/// A `pattern`/`id_pattern` inside an `enforce` block needs a human-readable
/// `description` (the regex is opaque to end users). Mirrors Python's
/// `validate_enforce_descriptions`.
fn validate_enforce_descriptions(out: &mut Vec<ValidationError>, config: &Config) {
    for (i, et) in config.entity_types.iter().enumerate() {
        if let Some(e) = &et.enforce {
            if e.id_pattern.is_some() && e.id_pattern_description.is_none() {
                out.push(err(
                    ErrorType::PatternMissingDescription,
                    format!("entity_types[{i}].enforce"),
                    format!("EntityType '{}' declares enforce.id_pattern but is missing 'id_pattern_description'", et.name),
                ));
            }
        }
    }
    for (i, lt) in config.link_types.iter().enumerate() {
        if let Some(e) = &lt.enforce {
            if e.id_pattern.is_some() && e.id_pattern_description.is_none() {
                out.push(err(
                    ErrorType::PatternMissingDescription,
                    format!("link_types[{i}].enforce"),
                    format!("LinkType '{}' declares enforce.id_pattern but is missing 'id_pattern_description'", lt.name),
                ));
            }
        }
    }
    for (i, ac) in config.attribute_classes.iter().enumerate() {
        if let Some(e) = &ac.enforce {
            if e.pattern.is_some() && e.description.is_none() {
                out.push(err(
                    ErrorType::PatternMissingDescription,
                    format!("attribute_classes[{i}].enforce"),
                    format!(
                        "AttributeClass '{}' declares enforce.pattern but is missing 'description'",
                        ac.name
                    ),
                ));
            }
        }
    }
}

fn validate_custom_icons_include(out: &mut Vec<ValidationError>, config: &Config) {
    if let Some(v) = config
        .settings
        .as_ref()
        .and_then(|s| s.extra_cfg.custom_icons_include.as_deref())
    {
        if v != "all" && v != "referenced" {
            out.push(err(
                ErrorType::InvalidCustomIconsInclude,
                "settings.extra_cfg.custom_icons_include",
                format!("must be 'all' or 'referenced', got '{v}'"),
            ));
        }
    }
}

fn validate_loose_cards(
    out: &mut Vec<ValidationError>,
    data: &ChartData,
    entity_ids: &HashMap<String, String>,
) {
    let link_ids: HashSet<&str> = data
        .links
        .iter()
        .filter_map(|l| l.link_id.as_deref())
        .collect();
    for (i, c) in data.loose_cards.iter().enumerate() {
        let loc = format!("loose_cards[{i}]");
        if let Some(eid) = c.entity_id.as_deref().filter(|s| !s.is_empty()) {
            if !entity_ids.contains_key(eid) {
                out.push(err(
                    ErrorType::MissingTarget,
                    &loc,
                    format!("loose card entity_id '{eid}' not found"),
                ));
            }
        }
        if let Some(lid) = c.link_id.as_deref().filter(|s| !s.is_empty()) {
            if !link_ids.contains(lid) {
                out.push(err(
                    ErrorType::MissingTarget,
                    &loc,
                    format!("loose card link_id '{lid}' not found"),
                ));
            }
        }
    }
}

type ConnStyle = (Option<String>, Option<i64>, Option<String>);

fn validate_connection_conflicts(out: &mut Vec<ValidationError>, links: &[crate::models::Link]) {
    use std::collections::HashMap;
    let mut seen: HashMap<(String, String), ConnStyle> = HashMap::new();
    for (i, l) in links.iter().enumerate() {
        if l.multiplicity.is_none() && l.fan_out.is_none() && l.theme_wiring.is_none() {
            continue;
        }
        let pair = if l.from_id <= l.to_id {
            (l.from_id.clone(), l.to_id.clone())
        } else {
            (l.to_id.clone(), l.from_id.clone())
        };
        let style = (l.multiplicity.clone(), l.fan_out, l.theme_wiring.clone());
        match seen.get(&pair) {
            Some(prev) if *prev != style => {
                out.push(err(
                    ErrorType::ConnectionConflict,
                    format!("links[{i}]"),
                    "links between the same pair have conflicting connection styles",
                ));
            }
            None => {
                seen.insert(pair, style);
            }
            _ => {}
        }
    }
}

/// A template references a placeholder if a single `{...}` remains after
/// stripping the escaped `{{`/`}}` pairs.
fn template_has_placeholder(t: &str) -> bool {
    let cleaned = t.replace("{{", "").replace("}}", "");
    cleaned
        .find('{')
        .is_some_and(|open| cleaned[open..].contains('}'))
}

/// Two display kinds overlap if either is `both` or they are equal.
fn kinds_overlap(a: &str, b: &str) -> bool {
    a == "both" || b == "both" || a == b
}

fn validate_display(out: &mut Vec<ValidationError>, config: &Config) {
    let Some(extra) = config.settings.as_ref().map(|s| &s.extra_cfg) else {
        return;
    };
    let ac_names: HashSet<&str> = config
        .attribute_classes
        .iter()
        .map(|a| a.name.as_str())
        .collect();
    // (base_loc, index, effective kind, type filter, slot) for overlap detection.
    // slot = attribute_name (attribute family) or "" (the single label slot).
    let mut parts: Vec<(&'static str, usize, String, Option<String>, String)> = Vec::new();

    for (i, d) in extra.display_attribute.iter().enumerate() {
        let base = "settings.extra_cfg.display_attribute";
        let loc = format!("{base}[{i}]");
        let key_ok = !d.key.as_deref().unwrap_or("").is_empty();
        if !key_ok {
            out.push(err(
                ErrorType::DisplayInvalid,
                &loc,
                "display entry needs 'key'",
            ));
        }
        let attr_name = d.attribute_name.as_deref().unwrap_or("");
        if attr_name.is_empty() {
            out.push(err(
                ErrorType::DisplayInvalid,
                &loc,
                "display_attribute needs 'attribute_name'",
            ));
        }
        let template = d.template.as_deref().unwrap_or("");
        if template.is_empty() {
            out.push(err(
                ErrorType::DisplayInvalid,
                &loc,
                "display entry needs 'template'",
            ));
        }
        let kind = d.kind.as_deref().unwrap_or("both");
        let kind_ok = ["entity", "link", "both"].contains(&kind);
        if !kind_ok {
            out.push(err(
                ErrorType::DisplayInvalid,
                &loc,
                format!("invalid kind '{kind}'"),
            ));
        }
        if d.sources.is_empty() && !template.is_empty() && template_has_placeholder(template) {
            out.push(err(
                ErrorType::DisplayInvalid,
                format!("{loc}.sources"),
                "'sources' is required because the template references placeholders",
            ));
        }
        if !attr_name.is_empty() && ac_names.contains(attr_name) {
            out.push(err(
                ErrorType::DisplayNameCollision,
                format!("{loc}.attribute_name"),
                format!("display_attribute[{i}] synthesized AC name '{attr_name}' collides with explicit AttributeClass"),
            ));
        }
        if key_ok && kind_ok && !attr_name.is_empty() {
            parts.push((
                base,
                i,
                kind.to_string(),
                d.r#type.clone(),
                attr_name.to_string(),
            ));
        }
    }
    for (i, d) in extra.display_label.iter().enumerate() {
        let base = "settings.extra_cfg.display_label";
        let loc = format!("{base}[{i}]");
        let key_ok = !d.key.as_deref().unwrap_or("").is_empty();
        if !key_ok {
            out.push(err(
                ErrorType::DisplayInvalid,
                &loc,
                "display entry needs 'key'",
            ));
        }
        let template = d.template.as_deref().unwrap_or("");
        if template.is_empty() {
            out.push(err(
                ErrorType::DisplayInvalid,
                &loc,
                "display entry needs 'template'",
            ));
        }
        let kind = d.kind.as_deref().unwrap_or("both");
        let kind_ok = ["entity", "link", "both"].contains(&kind);
        if !kind_ok {
            out.push(err(
                ErrorType::DisplayInvalid,
                &loc,
                format!("invalid kind '{kind}'"),
            ));
        }
        if d.sources.is_empty() && !template.is_empty() && template_has_placeholder(template) {
            out.push(err(
                ErrorType::DisplayInvalid,
                format!("{loc}.sources"),
                "'sources' is required because the template references placeholders",
            ));
        }
        if key_ok && kind_ok {
            parts.push((base, i, kind.to_string(), d.r#type.clone(), String::new()));
        }
    }
    // Overlap: same slot, intersecting kinds, same specificity tier.
    for a in 0..parts.len() {
        for b in (a + 1)..parts.len() {
            let (_, ia, ka, ta, sa) = &parts[a];
            let (lb, ib, kb, tb, sb) = &parts[b];
            if sa != sb || !kinds_overlap(ka, kb) {
                continue;
            }
            let same_tier = (ta.is_none() && tb.is_none()) || (ta.is_some() && ta == tb);
            if !same_tier {
                continue;
            }
            out.push(err(
                ErrorType::DisplayOverlapConflict,
                format!("{lb}[{ib}]"),
                format!("{lb}[{ib}] overlaps {lb}[{ia}]: both apply to the same slot"),
            ));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn check_chart_item_common(
    out: &mut Vec<ValidationError>,
    loc: &str,
    date: Option<&str>,
    time: Option<&str>,
    strength: Option<&str>,
    strength_names: &HashSet<&str>,
    g1: &Option<GradeRef>,
    g2: &Option<GradeRef>,
    g3: &Option<GradeRef>,
    g1_items: &[String],
    g2_items: &[String],
    g3_items: &[String],
    datetime_format: Option<&str>,
    dtf_names: &HashSet<&str>,
) {
    if let Some(f) = datetime_format.filter(|s| !s.is_empty()) {
        if !dtf_names.contains(f) {
            out.push(err(
                ErrorType::UnregisteredDatetimeFormat,
                loc,
                format!("datetime_format '{f}' is not registered in the DateTimeFormatCollection"),
            ));
        }
    }
    if let Some(d) = date.filter(|s| !s.is_empty()) {
        if parse_date(d).is_none() {
            out.push(err(
                ErrorType::InvalidDate,
                loc,
                format!("Invalid date '{d}'"),
            ));
        }
    }
    if let Some(t) = time.filter(|s| !s.is_empty()) {
        if parse_time(t).is_none() {
            out.push(err(
                ErrorType::InvalidTime,
                loc,
                format!("Invalid time '{t}'"),
            ));
        }
    }
    if let Some(s) = strength.filter(|s| !s.is_empty()) {
        if !strength_names.contains(s) {
            out.push(err(
                ErrorType::InvalidStrength,
                loc,
                format!("Strength '{s}' not found in chart.strengths"),
            ));
        }
    }
    check_grade(out, g1, g1_items, "grade_one", loc);
    check_grade(out, g2, g2_items, "grade_two", loc);
    check_grade(out, g3, g3_items, "grade_three", loc);
}

fn check_enum(
    out: &mut Vec<ValidationError>,
    val: &Option<String>,
    valid: &[&str],
    t: ErrorType,
    field: &str,
    loc: &str,
) {
    if let Some(v) = val.as_deref().filter(|s| !s.is_empty()) {
        if !valid.contains(&v) {
            out.push(err(t, loc.to_string(), format!("Invalid {field} '{v}'")));
        }
    }
}

fn validate_type_defs(out: &mut Vec<ValidationError>, config: &Config) {
    let mut et_names = HashSet::new();
    for (i, et) in config.entity_types.iter().enumerate() {
        let loc = format!("entity_types[{i}]");
        if et.name.is_empty() {
            out.push(err(
                ErrorType::MissingRequired,
                &loc,
                "Entity type missing 'name'",
            ));
        } else if !et_names.insert(et.name.clone()) {
            out.push(err(
                ErrorType::DuplicateName,
                &loc,
                format!("Duplicate entity type '{}'", et.name),
            ));
        }
        check_color(out, &et.color, "color", &loc);
        check_color(out, &et.shade_color, "shade_color", &loc);
    }
    let mut lt_names = HashSet::new();
    for (i, lt) in config.link_types.iter().enumerate() {
        let loc = format!("link_types[{i}]");
        if lt.name.is_empty() {
            out.push(err(
                ErrorType::MissingRequired,
                &loc,
                "Link type missing 'name'",
            ));
        } else if !lt_names.insert(lt.name.clone()) {
            out.push(err(
                ErrorType::DuplicateName,
                &loc,
                format!("Duplicate link type '{}'", lt.name),
            ));
        }
        check_color(out, &lt.color, "color", &loc);
    }
    let mut ac_names = HashSet::new();
    for (i, ac) in config.attribute_classes.iter().enumerate() {
        let loc = format!("attribute_classes[{i}]");
        if ac.name.is_empty() {
            out.push(err(
                ErrorType::MissingRequired,
                &loc,
                "Attribute class missing 'name'",
            ));
        } else if !ac_names.insert(ac.name.clone()) {
            out.push(err(
                ErrorType::DuplicateName,
                &loc,
                format!("Duplicate attribute class '{}'", ac.name),
            ));
        }
        // DateTime attribute classes cannot be canvas-visible (ANB v9 guard).
        if ac.r#type == Some(crate::enums::AttributeType::Datetime) && ac.visible == Some(true) {
            out.push(err(
                ErrorType::DatetimeAcForbidsVisible,
                &loc,
                "A DateTime attribute class cannot have visible=true",
            ));
        }
    }
}

fn collect_entities(data: &ChartData) -> Vec<Entity> {
    let g = &data.entities;
    let mut out = Vec::new();
    out.extend(g.icons.iter().cloned().map(Entity::Icon));
    out.extend(g.boxes.iter().cloned().map(Entity::Box));
    out.extend(g.circles.iter().cloned().map(Entity::Circle));
    out.extend(g.theme_lines.iter().cloned().map(Entity::ThemeLine));
    out.extend(g.event_frames.iter().cloned().map(Entity::EventFrame));
    out.extend(g.text_blocks.iter().cloned().map(Entity::TextBlock));
    out.extend(g.labels.iter().cloned().map(Entity::Label));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_from(json: &str) -> ChartData {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn clean_example_has_no_errors() {
        let config: Config =
            serde_json::from_str(include_str!("../tests/fixtures/example_config.json")).unwrap();
        let data: ChartData =
            serde_json::from_str(include_str!("../tests/fixtures/example_data.json")).unwrap();
        let errs = validate(&config, &data);
        assert!(errs.is_empty(), "unexpected errors: {errs:?}");
    }

    #[test]
    fn catches_dangling_link_and_self_loop() {
        let data = data_from(
            r#"{"entities":{"icons":[{"id":"a","type":"P"}]},
                "links":[{"from_id":"a","to_id":"ghost"},{"from_id":"a","to_id":"a"}]}"#,
        );
        let errs = validate(&Config::default(), &data);
        // A dangling endpoint (from_id or to_id) is `missing_entity`, matching
        // Python; `missing_target` is reserved for loose cards.
        assert!(errs
            .iter()
            .any(|e| e.error_type == ErrorType::MissingEntity));
        assert!(errs.iter().any(|e| e.error_type == ErrorType::SelfLoop));
    }

    #[test]
    fn catches_duplicate_id_and_missing_type() {
        let data =
            data_from(r#"{"entities":{"icons":[{"id":"a","type":"P"},{"id":"a"}]},"links":[]}"#);
        let errs = validate(&Config::default(), &data);
        assert!(errs.iter().any(|e| e.error_type == ErrorType::DuplicateId));
        assert!(errs
            .iter()
            .any(|e| e.error_type == ErrorType::MissingRequired));
    }

    #[test]
    fn catches_bad_arrow_and_color() {
        let data = data_from(
            r#"{"entities":{"icons":[{"id":"a","type":"P"},{"id":"b","type":"P"}]},
                "links":[{"from_id":"a","to_id":"b","arrow":"sideways","line_color":"notacolor"}]}"#,
        );
        let errs = validate(&Config::default(), &data);
        assert!(errs.iter().any(|e| e.error_type == ErrorType::InvalidArrow));
        assert!(errs.iter().any(|e| e.error_type == ErrorType::UnknownColor));
    }
}
