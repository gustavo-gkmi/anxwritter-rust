//! Data transforms applied during resolution, mirroring `anxwritter/transforms.py`.
//!
//! Ported so far: HSV auto-colouring of entities without an explicit colour, and
//! grade name/default resolution. Geo-map, icon-map, display synthesizers, and
//! link styling come later.

use indexmap::IndexMap;

use crate::color::{rgb_to_colorref, ColorValue};
use crate::entities::EntityRef;
use crate::models::{GradeCollection, GradeRef, Link};

/// Symmetric arc offsets for `n` parallel links (matches `_compute_symmetric_offsets`):
/// odd → `0, +s, -s, +2s, -2s, …`; even → `+s/2, -s/2, +3s/2, -3s/2, …`.
fn symmetric_offsets(n: usize, spacing: i64) -> Vec<i64> {
    if n <= 1 {
        return vec![0; n];
    }
    let half = spacing / 2;
    let mut out = Vec::with_capacity(n);
    if n.is_multiple_of(2) {
        for k in 0..(n / 2) {
            let mag = half + k as i64 * spacing;
            out.push(mag);
            out.push(-mag);
        }
    } else {
        out.push(0);
        for k in 1..n.div_ceil(2) {
            let mag = k as i64 * spacing;
            out.push(mag);
            out.push(-mag);
        }
    }
    out
}

/// Auto arc-offset per link index: links sharing a directed `(from,to)` pair are
/// fanned out symmetrically so they don't overlap. Mirrors `compute_link_offsets`.
pub fn compute_link_offsets(links: &[Link], spacing: i64) -> Vec<i64> {
    let mut groups: IndexMap<(&str, &str), Vec<usize>> = IndexMap::new();
    for (i, l) in links.iter().enumerate() {
        if !l.from_id.is_empty() && !l.to_id.is_empty() && l.from_id != l.to_id {
            groups
                .entry((l.from_id.as_str(), l.to_id.as_str()))
                .or_default()
                .push(i);
        }
    }
    let mut offsets = vec![0i64; links.len()];
    for indices in groups.values() {
        for (idx, off) in indices
            .iter()
            .zip(symmetric_offsets(indices.len(), spacing))
        {
            offsets[*idx] = off;
        }
    }
    offsets
}

/// `colorsys.hsv_to_rgb` — h, s, v in `[0, 1]` -> r, g, b in `[0, 1]`.
pub fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (f64, f64, f64) {
    if s == 0.0 {
        return (v, v, v);
    }
    let i = (h * 6.0) as i64; // truncates toward zero; h >= 0
    let f = h * 6.0 - i as f64;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match i.rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

/// The explicit shading colour the auto-colourer treats as "already set":
/// `color` on Icon, `shade_color` on ThemeLine/EventFrame, nothing elsewhere
/// (matching the Python `getattr(e,'color') or getattr(e,'shade_color')`).
fn explicit_shade<'a>(e: EntityRef<'a>) -> Option<&'a ColorValue> {
    match e {
        EntityRef::Icon(i) => i.color.as_ref(),
        EntityRef::ThemeLine(t) => t.shade_color.as_ref(),
        EntityRef::EventFrame(ev) => ev.shade_color.as_ref(),
        _ => None,
    }
}

/// Auto-assigned colours per entity id: `(bg_colorref, fg_colorref)`.
///
/// Entities without an explicit shade colour get evenly-spaced HSV hues
/// (s=0.55, v=0.90); `fg` is black on light backgrounds, white on dark.
pub fn compute_auto_colors(entities: &[EntityRef]) -> IndexMap<String, (u32, u32)> {
    // First-seen explicit colour per id.
    let mut seen: IndexMap<String, bool> = IndexMap::new(); // id -> has_explicit
    for e in entities {
        let id = &e.common().id;
        if id.is_empty() {
            continue;
        }
        if !seen.contains_key(id) {
            seen.insert(id.clone(), explicit_shade(*e).is_some());
        }
    }
    let uncolored: Vec<String> = seen
        .iter()
        .filter(|(_, has)| !**has)
        .map(|(id, _)| id.clone())
        .collect();
    let n = uncolored.len();
    let mut out = IndexMap::new();
    if n == 0 {
        return out;
    }
    for (idx, id) in uncolored.iter().enumerate() {
        let hue = if n > 1 { idx as f64 / n as f64 } else { 0.0 };
        let (rf, gf, bf) = hsv_to_rgb(hue, 0.55, 0.90);
        let (r, g, b) = (
            (rf * 255.0) as u32,
            (gf * 255.0) as u32,
            (bf * 255.0) as u32,
        );
        let bg = rgb_to_colorref(r, g, b);
        let luminance = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
        let fg = if luminance > 128.0 { 0 } else { 16777215 };
        out.insert(id.clone(), (bg, fg));
    }
    out
}

/// Canonical comparison key: trim, optionally case-fold. (Accent folding is a
/// best-effort subset — ASCII passes through unchanged.)
fn fold_key(s: &str, case: bool) -> String {
    let s = s.trim();
    if case {
        s.to_lowercase()
    } else {
        s.to_string()
    }
}

/// Does this entity representation carry an `icon` field (Icon/ThemeLine/EventFrame)?
fn supports_icon(e: EntityRef) -> bool {
    matches!(
        e,
        EntityRef::Icon(_) | EntityRef::ThemeLine(_) | EntityRef::EventFrame(_)
    )
}

/// Compute per-entity icon overrides from `extra_cfg.icon_map` rules.
///
/// Returns `entity_id -> icon_name`. Precedence by tier (id > typed-attribute >
/// untyped-attribute), last-wins within a tier. Only entities whose
/// representation has an icon are considered. The explicit-icon-wins rule is
/// applied by the caller.
pub fn apply_icon_map(
    entities: &[EntityRef],
    icon_map: &crate::models::IconMapCfg,
) -> IndexMap<String, String> {
    let mut out = IndexMap::new();
    if icon_map.rules.is_empty() {
        return out;
    }
    for e in entities {
        if !supports_icon(*e) {
            continue;
        }
        let c = e.common();
        if c.id.is_empty() {
            continue;
        }
        let mut best_tier = -1i32;
        let mut best_icon: Option<String> = None;
        for rule in &icon_map.rules {
            let tier = icon_rule_tier(rule);
            if tier < best_tier {
                continue;
            }
            if let Some(icon) = eval_icon_rule(rule, &c.id, &c.r#type, &c.attributes) {
                best_tier = tier;
                best_icon = Some(icon);
            }
        }
        if let Some(icon) = best_icon {
            out.insert(c.id.clone(), icon);
        }
    }
    out
}

fn icon_rule_tier(rule: &crate::models::IconRule) -> i32 {
    let m = rule
        .r#match
        .as_deref()
        .unwrap_or("attribute")
        .to_lowercase();
    if m == "id" {
        2
    } else if rule.r#type.is_some() {
        1
    } else {
        0
    }
}

fn eval_icon_rule(
    rule: &crate::models::IconRule,
    eid: &str,
    etype: &str,
    attrs: &IndexMap<String, crate::value::AttrValue>,
) -> Option<String> {
    let m = rule
        .r#match
        .as_deref()
        .unwrap_or("attribute")
        .to_lowercase();
    if m == "id" {
        return rule.mapping.get(eid).cloned();
    }
    let attr_name = rule.attribute_name.as_deref()?;
    if let Some(t) = &rule.r#type {
        if t != etype {
            return None;
        }
    }
    let strict = rule.strict_match.unwrap_or(false);
    let attr_norm = fold_key(attr_name, true);
    let mut value: Option<&crate::value::AttrValue> = None;
    for (k, v) in attrs {
        if fold_key(k, true) == attr_norm {
            value = Some(v);
            break;
        }
    }
    let Some(value) = value else {
        return rule.default_when_absent.clone();
    };
    let key = fold_key(&value.render(), !strict);
    // Mapping keys are folded the same way.
    for (k, v) in &rule.mapping {
        if fold_key(k, !strict) == key {
            return Some(v.clone());
        }
    }
    rule.default.clone()
}

/// Choose the categorical style for a link by its attribute value, falling back
/// to the configured `default`. Mirrors `apply_link_categorical`.
pub fn categorical_style<'a>(
    attrs: &IndexMap<String, crate::value::AttrValue>,
    ccfg: &'a crate::models::CategoricalCfg,
) -> Option<&'a crate::models::CategoricalStyleCfg> {
    let attr = ccfg.attribute.as_deref()?;
    if ccfg.styles.is_empty() {
        return None;
    }
    let fold_case = !ccfg.case_sensitive.unwrap_or(false);
    let raw = attrs.get(attr);
    match raw {
        None => ccfg.default.as_ref(),
        Some(v) => {
            let key = fold_key(&v.render(), fold_case);
            ccfg.styles
                .iter()
                .find(|(k, _)| fold_key(k, fold_case) == key)
                .map(|(_, s)| s)
                .or(ccfg.default.as_ref())
        }
    }
}

// ── Display synthesizers (template → sibling attribute / label) ─────────────

/// A resolved display source: which attribute feeds which template key.
pub struct DisplaySourceMeta {
    pub attribute: String,
    pub alias: String,
    pub missing: String,
    pub placeholder: String,
}

/// Flatten `DisplaySource`s into render metas (skipping sources with no attribute).
pub fn source_metas(sources: &[crate::models::DisplaySource]) -> Vec<DisplaySourceMeta> {
    sources
        .iter()
        .filter_map(|s| {
            let attribute = s.attribute.clone()?;
            Some(DisplaySourceMeta {
                alias: s.alias.clone().unwrap_or_else(|| attribute.clone()),
                attribute,
                missing: s.missing.clone().unwrap_or_else(|| "skip".into()),
                placeholder: s.placeholder.clone().unwrap_or_default(),
            })
        })
        .collect()
}

/// Render one item's template against a `class_name -> value` lookup. Returns
/// `None` to skip the item (a `skip`/`error` source was missing). Mirrors
/// `_render_display` + `_SeparatorFormatter`, including numeric format specs
/// (`,` grouping, `.Nf` precision, custom separators) and datetime specs (`%…`).
pub fn render_display(
    attr_lookup: &IndexMap<String, String>,
    template: &str,
    sources: &[DisplaySourceMeta],
    decimal_sep: &str,
    thousand_sep: &str,
) -> Option<String> {
    let mut fmt: IndexMap<String, String> = IndexMap::new();
    for s in sources {
        match attr_lookup.get(&s.attribute) {
            None => {
                if s.missing == "skip" || s.missing == "error" {
                    return None;
                }
                fmt.insert(s.alias.clone(), s.placeholder.clone());
            }
            Some(v) => {
                fmt.insert(s.alias.clone(), v.clone());
            }
        }
    }
    Some(substitute_template(
        template,
        &fmt,
        decimal_sep,
        thousand_sep,
    ))
}

/// Substitute `{key}` / `{key:spec}` placeholders; `{{`/`}}` are literal braces.
fn substitute_template(
    template: &str,
    fmt: &IndexMap<String, String>,
    dec: &str,
    thou: &str,
) -> String {
    let mut out = String::new();
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                let mut field = String::new();
                for c in chars.by_ref() {
                    if c == '}' {
                        break;
                    }
                    field.push(c);
                }
                let (key, spec) = match field.split_once(':') {
                    Some((k, s)) => (k.trim(), s),
                    None => (field.trim(), ""),
                };
                if let Some(v) = fmt.get(key) {
                    out.push_str(&format_value(v, spec, dec, thou));
                }
            }
            c => out.push(c),
        }
    }
    out
}

/// Apply a Python-style format spec to a value (numeric grouping/precision or
/// `strftime` datetime). Unknown/empty specs render the value verbatim.
fn format_value(val: &str, spec: &str, dec: &str, thou: &str) -> String {
    if spec.is_empty() {
        return val.to_string();
    }
    if spec.contains('%') {
        // Datetime spec.
        if let Some((y, m, d)) = crate::datetime::parse_date(val) {
            let (h, mi, s) = val
                .split(['T', ' '])
                .nth(1)
                .and_then(crate::datetime::parse_time)
                .unwrap_or((0, 0, 0));
            if let Some(dt) = chrono::NaiveDate::from_ymd_opt(y as i32, m, d)
                .and_then(|nd| nd.and_hms_opt(h, mi, s))
            {
                return dt.format(spec).to_string();
            }
        }
        return val.to_string();
    }
    // Numeric spec: optional grouping ',', optional '.Nf' precision.
    let Ok(num) = val.parse::<f64>() else {
        return val.to_string();
    };
    let grouping = spec.contains(',');
    let precision = spec.rsplit_once('.').and_then(|(_, p)| {
        p.trim_end_matches(|c: char| c.is_ascii_alphabetic())
            .parse::<usize>()
            .ok()
    });
    let mut s = match precision {
        Some(p) => format!("{num:.p$}"),
        None if num.fract() == 0.0 => format!("{}", num as i64),
        None => num.to_string(),
    };
    if grouping {
        let neg = s.starts_with('-');
        let body = s.trim_start_matches('-');
        let (int_part, frac) = match body.split_once('.') {
            Some((i, f)) => (i, Some(f)),
            None => (body, None),
        };
        let grouped = group_thousands(int_part, thou);
        s = match frac {
            Some(f) => format!("{grouped}.{f}"),
            None => grouped,
        };
        if neg {
            s.insert(0, '-');
        }
    }
    if dec != "." {
        if let Some(pos) = s.rfind('.') {
            s.replace_range(pos..pos + 1, dec);
        }
    }
    s
}

fn group_thousands(int_part: &str, sep: &str) -> String {
    let digits: Vec<char> = int_part.chars().collect();
    let mut out = String::new();
    for (i, c) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push_str(sep);
        }
        out.push(*c);
    }
    out
}

/// Whether a display `kind` filter applies to an item.
pub fn display_kind_matches(kind: Option<&str>, is_link: bool) -> bool {
    let kind = kind.unwrap_or("both");
    if is_link {
        kind != "entity"
    } else {
        kind != "link"
    }
}

// ── Geo-map (lat/lon → canvas XY) ───────────────────────────────────────────

fn norm_geo_key(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Match entities to geo data, returning `entity_id -> (lat, lon)` for matches.
pub fn geo_coords(
    entities: &[EntityRef],
    geo: &crate::models::GeoMapCfg,
) -> IndexMap<String, (f64, f64)> {
    let mut out = IndexMap::new();
    let (Some(attr), Some(data)) = (geo.attribute_name.as_deref(), geo.data.as_ref()) else {
        return out;
    };
    let attr = norm_geo_key(attr);
    let mut geo_data: IndexMap<String, (f64, f64)> = IndexMap::new();
    for (k, v) in data {
        if v.len() >= 2 {
            geo_data.insert(norm_geo_key(k), (v[0], v[1]));
        }
    }
    for e in entities {
        let c = e.common();
        let val = c
            .attributes
            .iter()
            .find(|(k, _)| norm_geo_key(k) == attr)
            .map(|(_, v)| v);
        if let Some(val) = val {
            if let Some(&coords) = geo_data.get(&norm_geo_key(&val.render())) {
                out.insert(c.id.clone(), coords);
            }
        }
    }
    out
}

/// Project entities matched to geo data onto a `width x height` canvas via
/// equirectangular projection (10% padding, Y inverted). Mirrors
/// `match_geo_entities` + `compute_geo_positions`.
pub fn geo_positions(
    entities: &[EntityRef],
    geo: &crate::models::GeoMapCfg,
) -> IndexMap<String, (i64, i64)> {
    let mut out = IndexMap::new();
    let attr = match geo.attribute_name.as_deref() {
        Some(a) => norm_geo_key(a),
        None => return out,
    };
    let data = match &geo.data {
        Some(d) => d,
        None => return out,
    };
    // Normalised geo key -> (lat, lon).
    let mut geo_data: IndexMap<String, (f64, f64)> = IndexMap::new();
    for (k, v) in data {
        if v.len() >= 2 {
            geo_data.insert(norm_geo_key(k), (v[0], v[1]));
        }
    }

    // Match entities to geo keys, grouped (entries share lat/lon).
    let mut matched: IndexMap<String, Vec<(String, f64, f64)>> = IndexMap::new();
    for e in entities {
        let c = e.common();
        let val = c
            .attributes
            .iter()
            .find(|(k, _)| norm_geo_key(k) == attr)
            .map(|(_, v)| v);
        let Some(val) = val else { continue };
        let nv = norm_geo_key(&val.render());
        if let Some(&(lat, lon)) = geo_data.get(&nv) {
            matched
                .entry(nv)
                .or_default()
                .push((c.id.clone(), lat, lon));
        }
    }
    if matched.is_empty() {
        return out;
    }

    let all: Vec<&(String, f64, f64)> = matched.values().flatten().collect();
    let (mut min_lat, mut max_lat) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut min_lon, mut max_lon) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in &all {
        min_lat = min_lat.min(p.1);
        max_lat = max_lat.max(p.1);
        min_lon = min_lon.min(p.2);
        max_lon = max_lon.max(p.2);
    }
    let lat_range0 = if max_lat - min_lat == 0.0 {
        1.0
    } else {
        max_lat - min_lat
    };
    let lon_range0 = if max_lon - min_lon == 0.0 {
        1.0
    } else {
        max_lon - min_lon
    };
    min_lat -= lat_range0 * 0.1;
    max_lat += lat_range0 * 0.1;
    min_lon -= lon_range0 * 0.1;
    max_lon += lon_range0 * 0.1;
    let lat_range = max_lat - min_lat;
    let lon_range = max_lon - min_lon;
    let width = geo.width.unwrap_or(3000) as f64;
    let height = geo.height.unwrap_or(2000) as f64;
    let spread = geo.spread_radius.unwrap_or(0);

    for entries in matched.values() {
        let (_, lat, lon) = entries[0];
        let cx = ((lon - min_lon) / lon_range * width) as i64;
        let cy = ((max_lat - lat) / lat_range * height) as i64; // Y inverted
        let n = entries.len();
        for (idx, (eid, _, _)) in entries.iter().enumerate() {
            let (ex, ey) = if spread > 0 && n > 1 {
                let angle = 2.0 * std::f64::consts::PI * idx as f64 / n as f64;
                (
                    cx + (spread as f64 * angle.cos()) as i64,
                    cy + (spread as f64 * angle.sin()) as i64,
                )
            } else {
                (cx, cy)
            };
            out.insert(eid.clone(), (ex, ey));
        }
    }
    out
}

// ── Intensity (numeric-attribute → width/color) link styling ────────────────

/// A flattened intensity sub-block (width or color), inheriting from the parent.
struct IntensitySub {
    attribute: Option<String>,
    scale: String,
    domain: Option<crate::models::IntensityDomain>,
    clip: bool,
    power: f64,
    range: Option<Vec<i64>>,
    ramp: Option<Vec<crate::color::ColorValue>>,
    space: String,
    diverging: bool,
    midpoint: Option<f64>,
}

fn numeric(v: &crate::value::AttrValue) -> Option<f64> {
    match v {
        crate::value::AttrValue::Int(i) => Some(*i as f64),
        crate::value::AttrValue::Float(f) if !f.is_nan() => Some(*f),
        _ => None,
    }
}

fn resolve_domain(values: &[f64], domain: &Option<crate::models::IntensityDomain>) -> (f64, f64) {
    use crate::models::IntensityDomain;
    if values.is_empty() {
        return (0.0, 1.0);
    }
    let (mut mn, mut mx) = (f64::INFINITY, f64::NEG_INFINITY);
    for &v in values {
        mn = mn.min(v);
        mx = mx.max(v);
    }
    match domain {
        Some(IntensityDomain::Range([a, b])) => (*a, *b),
        Some(IntensityDomain::Keyword(k)) if k == "robust" => {
            let mut s = values.to_vec();
            s.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let lo = percentile(&s, 5.0);
            let hi = percentile(&s, 95.0);
            if (lo - hi).abs() < f64::EPSILON {
                (mn, mx)
            } else {
                (lo, hi)
            }
        }
        _ => (mn, mx),
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = p / 100.0 * (sorted.len() as f64 - 1.0);
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        sorted[lo] + (sorted[hi] - sorted[lo]) * (rank - lo as f64)
    }
}

/// Map a value to `t` in `[0, 1]` via the requested scale (`apply_scale`).
fn apply_scale(
    mut v: f64,
    lo: f64,
    hi: f64,
    scale: &str,
    power: f64,
    clip: bool,
    sorted: &[f64],
) -> f64 {
    if clip {
        v = v.clamp(lo, hi);
    }
    if hi <= lo {
        return 0.5;
    }
    let t = match scale {
        "linear" => (v - lo) / (hi - lo),
        "log" => {
            if v <= 0.0 || lo <= 0.0 {
                0.0
            } else {
                (v.ln() - lo.ln()) / (hi.ln() - lo.ln())
            }
        }
        "sqrt" => (v.max(lo) - lo).sqrt() / (hi - lo).sqrt(),
        "power" => ((v.max(lo) - lo) / (hi - lo)).powf(power),
        "quantile" => {
            if sorted.len() <= 1 {
                0.5
            } else {
                let lo_rank = bisect_left(sorted, v);
                let hi_rank = bisect_right(sorted, v) as i64 - 1;
                if hi_rank < 0 {
                    0.0
                } else {
                    let avg = (lo_rank as f64 + hi_rank as f64) / 2.0;
                    avg / (sorted.len() as f64 - 1.0)
                }
            }
        }
        _ => 0.0,
    };
    t.clamp(0.0, 1.0)
}

fn bisect_left(s: &[f64], v: f64) -> usize {
    s.partition_point(|&x| x < v)
}
fn bisect_right(s: &[f64], v: f64) -> usize {
    s.partition_point(|&x| x <= v)
}

fn diverging_t(mut v: f64, lo: f64, mid: f64, hi: f64, clip: bool) -> f64 {
    if clip {
        v = v.clamp(lo, hi);
    }
    if v <= mid {
        if mid <= lo {
            return 0.0;
        }
        (0.5 * (v - lo) / (mid - lo)).clamp(0.0, 0.5)
    } else {
        if hi <= mid {
            return 1.0;
        }
        (0.5 + 0.5 * (v - mid) / (hi - mid)).clamp(0.5, 1.0)
    }
}

/// Compute per-link `(line_width, line_color)` overrides from an intensity
/// config. Returns one entry per link (caller applies only where the link left
/// the field unset).
pub fn intensity_overrides(
    links: &[crate::models::Link],
    icfg: &crate::models::IntensityCfg,
) -> Vec<(Option<i64>, Option<u32>)> {
    let mut out = vec![(None, None); links.len()];
    if let Some(sub) = resolve_sub(icfg, true) {
        apply_intensity_sub(links, &sub, true, &mut out);
    }
    if let Some(sub) = resolve_sub(icfg, false) {
        apply_intensity_sub(links, &sub, false, &mut out);
    }
    out
}

fn resolve_sub(icfg: &crate::models::IntensityCfg, is_width: bool) -> Option<IntensitySub> {
    if is_width {
        let w = icfg.width.as_ref()?;
        Some(IntensitySub {
            attribute: w.attribute.clone().or_else(|| icfg.attribute.clone()),
            scale: w
                .scale
                .clone()
                .or_else(|| icfg.scale.clone())
                .unwrap_or_else(|| "sqrt".into()),
            domain: w.domain.clone().or_else(|| icfg.domain.clone()),
            clip: w.clip.or(icfg.clip).unwrap_or(true),
            power: w.power.unwrap_or(0.5),
            range: w.range.clone(),
            ramp: None,
            space: "rgb_linear".into(),
            diverging: false,
            midpoint: None,
        })
    } else {
        let c = icfg.color.as_ref()?;
        Some(IntensitySub {
            attribute: c.attribute.clone().or_else(|| icfg.attribute.clone()),
            scale: c
                .scale
                .clone()
                .or_else(|| icfg.scale.clone())
                .unwrap_or_else(|| "sqrt".into()),
            domain: c.domain.clone().or_else(|| icfg.domain.clone()),
            clip: c.clip.or(icfg.clip).unwrap_or(true),
            power: c.power.unwrap_or(0.5),
            range: None,
            ramp: c.ramp.clone(),
            space: c.space.clone().unwrap_or_else(|| "rgb_linear".into()),
            diverging: c.diverging.unwrap_or(false),
            midpoint: c.midpoint,
        })
    }
}

fn apply_intensity_sub(
    links: &[crate::models::Link],
    cfg: &IntensitySub,
    is_width: bool,
    out: &mut [(Option<i64>, Option<u32>)],
) {
    let Some(attr) = cfg.attribute.as_deref() else {
        return;
    };
    let mut values = Vec::new();
    for l in links {
        if let Some(v) = l.attributes.get(attr).and_then(numeric) {
            values.push(v);
        }
    }
    if values.is_empty() {
        return;
    }
    let mut sorted = values.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let (lo, hi) = resolve_domain(&values, &cfg.domain);

    let ramp_ints: Vec<u32> = cfg
        .ramp
        .as_ref()
        .map(|r| r.iter().filter_map(|c| c.to_colorref().ok()).collect())
        .unwrap_or_default();
    if !is_width && ramp_ints.len() < 2 {
        return;
    }

    for (i, l) in links.iter().enumerate() {
        let Some(v) = l.attributes.get(attr).and_then(numeric) else {
            continue;
        };
        let t = match cfg.midpoint {
            Some(mid) if !is_width && cfg.diverging => diverging_t(v, lo, mid, hi, cfg.clip),
            _ => apply_scale(v, lo, hi, &cfg.scale, cfg.power, cfg.clip, &sorted),
        };
        if is_width {
            if let Some(rng) = &cfg.range {
                if rng.len() == 2 {
                    let w = rng[0] as f64 + (rng[1] - rng[0]) as f64 * t;
                    out[i].0 = Some((w.round() as i64).max(0));
                }
            }
        } else {
            out[i].1 = Some(crate::color::interpolate_ramp(&ramp_ints, t, &cfg.space));
        }
    }
}

/// Resolve a grade reference to an index: an int passes through, a digit string
/// parses, a name is looked up in `names`, anything else is `None`.
pub fn resolve_grade(val: Option<&GradeRef>, names: &[String]) -> Option<i64> {
    match val? {
        GradeRef::Index(i) => Some(*i),
        GradeRef::Name(s) => {
            let s = s.trim();
            if let Ok(i) = s.parse::<i64>() {
                return Some(i);
            }
            if s.is_empty() {
                return None;
            }
            names.iter().position(|n| n == s).map(|p| p as i64)
        }
    }
}

/// The default grade index for a collection: the position of its `default` name,
/// or `None` if unset/unknown.
pub fn grade_default_index(gc: &GradeCollection) -> Option<i64> {
    let d = gc.default.as_deref()?;
    gc.items.iter().position(|n| n == d).map(|p| p as i64)
}

/// Resolve a grade with default fallback.
pub fn resolve_grade_with_default(val: Option<&GradeRef>, gc: &GradeCollection) -> Option<i64> {
    resolve_grade(val, &gc.items).or_else(|| grade_default_index(gc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{EntityCommon, Icon};

    fn icon(id: &str) -> Icon {
        Icon {
            common: EntityCommon {
                id: id.into(),
                r#type: "Person".into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn auto_color_first_hue_matches_upstream() {
        // idx 0 of 8 -> hue 0 -> COLORREF 6776805, light bg -> fg black.
        let icons: Vec<Icon> = (0..8).map(|i| icon(&format!("e{i}"))).collect();
        let ents: Vec<EntityRef> = icons.iter().map(EntityRef::Icon).collect();
        let colors = compute_auto_colors(&ents);
        assert_eq!(colors["e0"], (6776805, 0));
    }

    #[test]
    fn grade_name_resolves_to_index() {
        let names = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        assert_eq!(
            resolve_grade(Some(&GradeRef::Name("B".into())), &names),
            Some(1)
        );
        assert_eq!(resolve_grade(Some(&GradeRef::Index(2)), &names), Some(2));
        assert_eq!(
            resolve_grade(Some(&GradeRef::Name("Z".into())), &names),
            None
        );
    }

    #[test]
    fn grade_default_index_lookup() {
        let gc = GradeCollection {
            default: Some("B".into()),
            items: vec!["A".into(), "B".into()],
        };
        assert_eq!(grade_default_index(&gc), Some(1));
    }
}
