//! The `.anx` XML builder, mirroring `anxwritter/builder.py`.
//!
//! Two passes: a *resolve* pass mints ids and registers entity/link/attribute
//! types in the same order the Python builder uses (so, e.g., the minimal chart
//! reproduces upstream ids), then an *emit* pass walks the document in schema
//! order. The correctness bar is a valid file i2 opens, not byte-identical
//! output — but matching ids and positions falls out naturally where cheap.

use indexmap::IndexMap;

use crate::color::ColorValue;
use crate::entities::EntityRef;
use crate::ids::IdCounter;
use crate::input::{ChartData, Config};
use crate::layout;
use crate::models::{AttributeClass, EntityType, GradeCollection, LinkType, Settings};
use crate::resolved::{ResolvedAttr, ResolvedEntity};
use crate::value::AttrValue;
use crate::xml::{Attr, Writer};

const VERSION: &str = crate::TARGET_ANXWRITTER_VERSION;
const REPO_URL: &str = "https://github.com/gustavo-gkmi/anxwritter";

// ── Enum -> ANB token maps ──────────────────────────────────────────────────

fn dot_style_token(s: &str) -> &'static str {
    match s {
        "dashed" | "---" => "DotStyleDashed",
        "dash_dot" | "-." => "DotStyleDashDot",
        "dash_dot_dot" | "-.." => "DotStyleDashDotDot",
        "dotted" | "..." => "DotStyleDotted",
        _ => "DotStyleSolid",
    }
}

fn arrow_token(s: &str) -> Option<&'static str> {
    match s {
        "head" | "->" | "ArrowOnHead" => Some("ArrowOnHead"),
        "tail" | "<-" | "ArrowOnTail" => Some("ArrowOnTail"),
        "both" | "<->" | "ArrowOnBoth" => Some("ArrowOnBoth"),
        _ => None,
    }
}

fn attribute_type_token(s: &str) -> &'static str {
    match s {
        "number" => "AttNumber",
        "datetime" => "AttTime",
        "flag" => "AttFlag",
        _ => "AttText",
    }
}

fn representation_token(s: &str) -> &'static str {
    match s.to_lowercase().as_str() {
        "box" => "RepresentAsBox",
        "circle" => "RepresentAsCircle",
        "text_block" => "RepresentAsTextBlock",
        "label" => "RepresentAsBorder",
        "ole_object" => "RepresentAsOLE",
        _ => "RepresentAsIcon",
    }
}

fn merge_behaviour_token(s: &str) -> &'static str {
    match s {
        "assign" => "AttMergeAssign",
        "noop" => "AttMergeNoOp",
        "add_space" => "AttMergeAddWithSpace",
        "add_line_break" => "AttMergeAddWithLineBreak",
        "max" => "AttMergeMax",
        "min" => "AttMergeMin",
        "subtract" => "AttMergeSubtract",
        "subtract_swap" => "AttMergeSubtractSwap",
        "or" => "AttMergeOR",
        "and" => "AttMergeAND",
        "xor" => "AttMergeXOR",
        _ => "AttMergeAdd",
    }
}

fn enlargement_token(s: &str) -> String {
    match s.to_lowercase().as_str() {
        "half" => "ICEnlargeHalf",
        "double" => "ICEnlargeDouble",
        "triple" => "ICEnlargeTriple",
        "quadruple" => "ICEnlargeQuadruple",
        "single" => "ICEnlargeSingle",
        other => return other.to_string(),
    }
    .to_string()
}

fn text_align_token(s: &str) -> String {
    match s.to_lowercase().as_str() {
        "left" => "TextAlignLeft",
        "right" => "TextAlignRight",
        "centre" | "center" => "TextAlignCentre",
        other => return other.to_string(),
    }
    .to_string()
}

fn legend_item_type_token(s: &str) -> String {
    match s.to_lowercase().as_str() {
        "text" => "LegendItemTypeText",
        "icon" => "LegendItemTypeIcon",
        "attribute" => "LegendItemTypeAttribute",
        "line" => "LegendItemTypeLine",
        "link" => "LegendItemTypeLink",
        "timezone" => "LegendItemTypeTimeZone",
        "icon_frame" => "LegendItemTypeIconFrame",
        _ => "LegendItemTypeFont",
    }
    .to_string()
}

fn legend_arrange_token(s: &str) -> String {
    let mut c = s.chars();
    let titled = match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect::<String>(),
        None => String::new(),
    };
    format!("LegendArrangement{titled}")
}

/// Map a link `multiplicity` (alias or canonical) to its ANB token.
/// A grade collection with items but no `default` gets a trailing `-` sentinel,
/// which also becomes the default (matches Python's `_resolve_grade`).
fn grade_with_dash(mut gc: crate::models::GradeCollection) -> crate::models::GradeCollection {
    if !gc.items.is_empty() && gc.default.is_none() {
        gc.items.push("-".into());
        gc.default = Some("-".into());
    }
    gc
}

/// Map a link `multiplicity` (alias or canonical) to its ANB token.
fn multiplicity_token(s: &str) -> String {
    match s {
        "multiple" => "MultiplicityMultiple",
        "single" => "MultiplicitySingle",
        "directed" => "MultiplicityDirected",
        other => other,
    }
    .to_string()
}

/// Map a link `theme_wiring` (alias or canonical) to its ANB token.
fn theme_wiring_token(s: &str) -> String {
    match s {
        "keep_event" => "KeepsAtEventHeight",
        "return_theme" => "ReturnsToThemeHeight",
        "next_event" => "GoesToNextEventHeight",
        "no_diversion" => "NoDiversion",
        other => other,
    }
    .to_string()
}

fn b(v: bool) -> String {
    if v { "true" } else { "false" }.to_string()
}

/// Format an f64 like Python's `str()`: integral values keep a trailing `.0`.
fn format_f64(x: f64) -> String {
    if x.is_finite() && x.fract() == 0.0 {
        format!("{x:.1}")
    } else {
        x.to_string()
    }
}

/// Emit one `<lcx:Type>` / `<lcx:Property>` catalogue entry.
fn emit_lcx_type(
    w: &mut Writer,
    tag: &str,
    guid_attr: &'static str,
    parent_attr: &'static str,
    name_tag: &str,
    t: &crate::semantic::CatType,
) {
    let mut a: Vec<Attr> = vec![(guid_attr, t.guid.clone())];
    if let Some(p) = &t.parent_guid {
        a.push((parent_attr, p.clone()));
    }
    if t.abstract_ {
        a.push(("abstract", "true".to_string()));
    }
    w.open(tag, &a);
    w.text_element(name_tag, &t.name);
    if !t.synonyms.is_empty() || t.description.is_some() {
        w.open("Documentation", &[]);
        for s in &t.synonyms {
            w.text_element("lcx:Synonym", s);
        }
        w.text_element("Description", t.description.as_deref().unwrap_or(""));
        w.close("Documentation");
    }
    w.close(tag);
}

/// Emit a `<TimeZone UniqueID Name/>` on a ChartItem, if present.
fn emit_timezone(w: &mut Writer, tz: Option<&crate::models::TimeZone>) {
    if let Some(tz) = tz {
        w.empty(
            "TimeZone",
            &[("UniqueID", tz.id.to_string()), ("Name", tz.name.clone())],
        );
    }
}

fn resolve_color(c: &Option<ColorValue>) -> Option<u32> {
    c.as_ref().and_then(|v| v.to_colorref().ok())
}

// ── Registries ──────────────────────────────────────────────────────────────

struct AcEntry {
    id: String,
    type_token: String,
    cfg: AttributeClass,
    semantic_guid: Option<String>,
}

struct EtEntry {
    id: String,
    cfg: EntityType,
    semantic_guid: Option<String>,
}

struct LtEntry {
    id: String,
    cfg: LinkType,
    semantic_guid: Option<String>,
}

struct StrengthEntry {
    id: String,
    dot_token: String,
}

/// Minimal per-link state retained after the register pass. Everything else is
/// re-derived from the borrowed input `Link` during emit (lazy resolution), so
/// the full resolved-link set is never materialized — this keeps peak memory
/// bounded on link-heavy charts (mirrors Python's streaming "fusion").
/// Connection-style key: `(multiplicity, fan_out, theme_wiring)`.
type ConnKey = (Option<String>, Option<i64>, Option<String>);

struct LinkMeta {
    ci_id: String,
    semantic_guid: Option<String>,
    int_width: Option<i64>,
    int_color: Option<u32>,
    /// Auto/explicit arc offset (parallel-link fan-out).
    offset: i64,
    /// `<Connection>` id when the link carries multiplicity/fan_out/theme_wiring.
    conn_ref: Option<String>,
}

/// Builds `.anx` XML from a [`Config`] and [`ChartData`].
pub struct Builder {
    ids: IdCounter,
    settings: Settings,
    strengths: IndexMap<String, StrengthEntry>,
    strength_default: Option<String>,
    entity_types: IndexMap<String, EtEntry>,
    link_types: IndexMap<String, LtEntry>,
    attribute_classes: IndexMap<String, AcEntry>,
    datetime_formats: Vec<(String, String, String)>, // (id, name, format)
    grades_one: GradeCollection,
    grades_two: GradeCollection,
    grades_three: GradeCollection,
    source_types: Vec<String>,
    legend_items: Vec<crate::models::LegendItem>,
    /// User-defined palettes (when present, replace the auto palette).
    palettes: Vec<crate::models::Palette>,
    summary: crate::models::SummaryCfg,

    // config metadata not yet promoted to a registry entry
    cfg_entity_types: IndexMap<String, EntityType>,
    cfg_link_types: IndexMap<String, LinkType>,
    cfg_attribute_classes: IndexMap<String, AttributeClass>,

    resolved_entities: Vec<ResolvedEntity>,
    link_meta: Vec<LinkMeta>,
    /// Deduped connection styles `(multiplicity, fan_out, theme_wiring) -> id`,
    /// in mint order (for `<ConnectionCollection>`).
    connections: IndexMap<ConnKey, String>,
    entity_int_by_id: IndexMap<String, i64>,
    /// Effective shade colour per entity id, for link colour matching.
    entity_color_map: IndexMap<String, u32>,
    /// Minted ids for grade/source `<String>` elements (one, two, three).
    grade_ids: [Vec<String>; 3],
    source_ids: Vec<String>,
    resolver: crate::semantic::SemanticResolver,
    /// Prepared custom images: `(composite_id, datalength, base64_data, emitted_name)`.
    custom_images: Vec<(String, u32, String, String)>,
    custom_entity_icon_names: IndexMap<String, String>,
    custom_attribute_icon_names: IndexMap<String, String>,
    used_custom_images: std::collections::HashSet<String>,
    custom_icons_include: String,
}

impl Builder {
    /// Create a builder from a config layer (use [`Config::default`] for none).
    pub fn new(config: &Config) -> Self {
        let settings = config.settings.clone().unwrap_or_default();
        let mut b = Builder {
            ids: IdCounter::new(),
            summary: settings.summary.clone(),
            settings,
            strengths: IndexMap::new(),
            strength_default: None,
            entity_types: IndexMap::new(),
            link_types: IndexMap::new(),
            attribute_classes: IndexMap::new(),
            datetime_formats: Vec::new(),
            grades_one: grade_with_dash(config.grades_one.clone().unwrap_or_default()),
            grades_two: grade_with_dash(config.grades_two.clone().unwrap_or_default()),
            grades_three: grade_with_dash(config.grades_three.clone().unwrap_or_default()),
            source_types: config.source_types.clone(),
            legend_items: config.legend_items.clone(),
            palettes: config.palettes.clone(),
            cfg_entity_types: IndexMap::new(),
            cfg_link_types: IndexMap::new(),
            cfg_attribute_classes: IndexMap::new(),
            resolved_entities: Vec::new(),
            link_meta: Vec::new(),
            connections: IndexMap::new(),
            entity_int_by_id: IndexMap::new(),
            entity_color_map: IndexMap::new(),
            grade_ids: [Vec::new(), Vec::new(), Vec::new()],
            source_ids: Vec::new(),
            resolver: crate::semantic::SemanticResolver::new(config),
            custom_images: Vec::new(),
            custom_entity_icon_names: IndexMap::new(),
            custom_attribute_icon_names: IndexMap::new(),
            used_custom_images: std::collections::HashSet::new(),
            custom_icons_include: config
                .settings
                .as_ref()
                .and_then(|s| s.extra_cfg.custom_icons_include.clone())
                .unwrap_or_else(|| "referenced".to_string()),
        };

        // Prepare custom icons (convert images, encode payloads).
        b.prepare_custom_icons(
            &config.custom_entity_icons,
            crate::custom_icons::IconKind::Icon,
        );
        b.prepare_custom_icons(
            &config.custom_attribute_icons,
            crate::custom_icons::IconKind::Attribute,
        );

        // latlon geo-map injection needs the i2 geo property hierarchy registered.
        if let Some(g) = &b.settings.extra_cfg.geo_map {
            if g.mode.as_deref() != Some("position") {
                b.resolver.register_geo_properties();
            }
        }

        // Strengths: a `strengths` section defines the collection. When it has no
        // `default`, a fallback strength named "-" is synthesized first; with no
        // section at all the fallback is named "Default" (the minimal case).
        if let Some(sc) = &config.strengths {
            match &sc.default {
                Some(d) => b.strength_default = Some(d.clone()),
                None => {
                    let id = b.ids.next_id();
                    b.strengths.insert(
                        "-".to_string(),
                        StrengthEntry {
                            id,
                            dot_token: "DotStyleSolid".to_string(),
                        },
                    );
                    b.strength_default = Some("-".to_string());
                }
            }
            for s in &sc.items {
                if !b.strengths.contains_key(&s.name) {
                    let id = b.ids.next_id();
                    let token = dot_style_token(&serde_to_str(&s.dot_style));
                    b.strengths.insert(
                        s.name.clone(),
                        StrengthEntry {
                            id,
                            dot_token: token.to_string(),
                        },
                    );
                }
            }
        }
        if b.strengths.is_empty() {
            let id = b.ids.next_id();
            b.strengths.insert(
                "Default".to_string(),
                StrengthEntry {
                    id,
                    dot_token: "DotStyleSolid".to_string(),
                },
            );
            b.strength_default = Some("Default".to_string());
        }

        // Datetime formats (id order step 3).
        for f in &config.datetime_formats {
            let id = b.ids.next_id();
            b.datetime_formats
                .push((id, f.name.clone(), f.format.clone()));
        }

        // Stash config metadata for lazy registration.
        for et in &config.entity_types {
            b.cfg_entity_types.insert(et.name.clone(), et.clone());
        }
        for lt in &config.link_types {
            b.cfg_link_types.insert(lt.name.clone(), lt.clone());
        }
        for ac in &config.attribute_classes {
            b.cfg_attribute_classes.insert(ac.name.clone(), ac.clone());
        }

        b
    }

    /// Resolve and emit compact `.anx` XML for `data` (the file form).
    pub fn build(self, data: &ChartData) -> String {
        self.build_with(data, true)
    }

    /// Resolve and emit `.anx` XML to an in-memory string, choosing compact
    /// (file) or pretty (indented, matching Python's `to_xml(compact=False)`).
    pub fn build_with(mut self, data: &ChartData, compact: bool) -> String {
        self.finalize(data);
        let mut s = String::new();
        {
            let mut w = Writer::new(&mut s, compact);
            self.emit_into(&mut w, data);
        }
        s
    }

    /// Resolve, then **stream** the `.anx` (UTF-16 LE + BOM) directly to `w` with
    /// bounded peak memory — never materializing the full document. Mirrors
    /// Python's `to_anx(stream=True)`. Ideal for an HTTP response body.
    pub fn write_to<W: std::io::Write>(
        mut self,
        data: &ChartData,
        w: W,
        compact: bool,
    ) -> std::io::Result<()> {
        self.finalize(data);
        let mut sink = crate::xml::Utf16Sink::new(w)?;
        {
            let mut wr = Writer::new(&mut sink, compact);
            self.emit_into(&mut wr, data);
        }
        sink.finish()
    }

    /// Run the resolve/register passes that must precede emission.
    fn finalize(&mut self, data: &ChartData) {
        self.resolve_entities(data);
        self.register_links(data);
        self.expand_displays();
        self.register_remaining_config_types();
        // Grade/source <String> ids (id order step 6).
        for dim in 0..3 {
            let n = [&self.grades_one, &self.grades_two, &self.grades_three][dim]
                .items
                .len();
            self.grade_ids[dim] = (0..n).map(|_| self.ids.next_id()).collect();
        }
        self.source_ids = (0..self.source_types.len())
            .map(|_| self.ids.next_id())
            .collect();
    }

    // ── Resolve pass ────────────────────────────────────────────────────────

    fn resolve_entities(&mut self, data: &ChartData) {
        let entities = collect_entity_refs(data);

        // Auto-colour map (HSV) when enabled.
        let auto_colors = if self.settings.extra_cfg.entity_auto_color == Some(true) {
            crate::transforms::compute_auto_colors(&entities)
        } else {
            IndexMap::new()
        };
        // Icon-map overrides from extra_cfg.icon_map.
        let icon_map_result = match &self.settings.extra_cfg.icon_map {
            Some(im) => crate::transforms::apply_icon_map(&entities, im),
            None => IndexMap::new(),
        };

        // Geo-map positions (position/both modes) take precedence over layout.
        let geo_positions = match &self.settings.extra_cfg.geo_map {
            Some(g) if g.mode.as_deref() != Some("latlon") => {
                crate::transforms::geo_positions(&entities, g)
            }
            _ => IndexMap::new(),
        };
        // Geo lat/lon attribute injection (latlon/both modes).
        let geo_latlon = match &self.settings.extra_cfg.geo_map {
            Some(g) if g.mode.as_deref() != Some("position") => {
                crate::transforms::geo_coords(&entities, g)
            }
            _ => IndexMap::new(),
        };
        let (lat_ref, lon_ref) = if geo_latlon.is_empty() {
            (None, None)
        } else {
            (
                Some(self.register_geo_ac("Latitude")),
                Some(self.register_geo_ac("Longitude")),
            )
        };

        // Assign per-entity int ids (1..) and gather layout inputs.
        let mut auto_keys = Vec::new();
        // ThemeLine entities are not geometrically laid out (they span the
        // timeline); they stay at the origin and are excluded from the key set.
        let all_keys: Vec<String> = entities
            .iter()
            .filter(|e| !matches!(e, EntityRef::ThemeLine(_)))
            .map(|e| e.common().id.clone())
            .collect();
        for (i, e) in entities.iter().enumerate() {
            self.entity_int_by_id
                .insert(e.common().id.clone(), (i + 1) as i64);
            let c = e.common();
            // Entities with manual or geo positions skip auto-layout.
            if !matches!(e, EntityRef::ThemeLine(_))
                && c.x.is_none()
                && c.y.is_none()
                && !geo_positions.contains_key(&c.id)
            {
                auto_keys.push(c.id.clone());
            }
        }
        let edges: Vec<(String, String)> = data
            .links
            .iter()
            .map(|l| (l.from_id.clone(), l.to_id.clone()))
            .collect();
        let arrange = self
            .settings
            .extra_cfg
            .arrange
            .clone()
            .unwrap_or_else(|| "radial".into());
        let scale = self.settings.extra_cfg.layout_scale.unwrap_or(1.0);
        let positions = layout::place(&arrange, &all_keys, &auto_keys, &edges, (0, 0), scale);

        for e in entities {
            let common = e.common().clone();
            let ci_id = self.ids.next_id();
            let etype_ref = self.register_entity_type(&common.r#type);
            let mut attrs = self.resolve_attrs(&common.attributes);
            // Inject geo lat/lon attributes for matched entities.
            if let Some(&(lat, lon)) = geo_latlon.get(&common.id) {
                attrs.push(ResolvedAttr {
                    class: "Latitude".to_string(),
                    reference: lat_ref.clone().unwrap(),
                    value: Some(format_f64(lat)),
                });
                attrs.push(ResolvedAttr {
                    class: "Longitude".to_string(),
                    reference: lon_ref.clone().unwrap(),
                    value: Some(format_f64(lon)),
                });
            }
            let (x, y) = if let (Some(mx), Some(my)) = (common.x, common.y) {
                (mx, my)
            } else if let Some(p) = geo_positions.get(&common.id) {
                *p
            } else {
                positions.get(&common.id).copied().unwrap_or((0, 0))
            };
            let label = common.label.clone().unwrap_or_else(|| common.id.clone());
            let int_id = self.entity_int_by_id[&common.id];

            // Grades: resolve names/indices, then fall back to configured defaults.
            let grade_one = crate::transforms::resolve_grade_with_default(
                common.grade_one.as_ref(),
                &self.grades_one,
            );
            let grade_two = crate::transforms::resolve_grade_with_default(
                common.grade_two.as_ref(),
                &self.grades_two,
            );
            let grade_three = crate::transforms::resolve_grade_with_default(
                common.grade_three.as_ref(),
                &self.grades_three,
            );

            // Auto-colour: shade (shade-reprs only) + label fg/bg for CIStyle.
            let (mut auto_shade, mut label_bg, mut label_fg) = (None, None, None);
            if let Some(&(bg, fg)) = auto_colors.get(&common.id) {
                if matches!(
                    e,
                    EntityRef::Icon(_) | EntityRef::ThemeLine(_) | EntityRef::EventFrame(_)
                ) {
                    auto_shade = Some(bg);
                }
                label_bg = Some(bg);
                label_fg = Some(fg);
            }

            // Effective shade colour (explicit or auto) for link colour matching.
            let explicit_shade = match e {
                EntityRef::Icon(i) => resolve_color(&i.color),
                EntityRef::ThemeLine(t) => resolve_color(&t.shade_color),
                EntityRef::EventFrame(ev) => resolve_color(&ev.shade_color),
                _ => None,
            };
            if let Some(c) = explicit_shade.or(auto_shade) {
                self.entity_color_map.insert(common.id.clone(), c);
            }

            let cards =
                self.resolve_cards(&common.cards, &data.loose_cards, Some(&common.id), false);
            let semantic_guid = self
                .resolver
                .resolve_type_name(common.semantic_type.as_deref());

            // Icon override: explicit `icon` field wins over the icon-map result.
            let explicit_icon = match e {
                EntityRef::Icon(i) => i.icon.clone(),
                EntityRef::ThemeLine(t) => t.icon.clone(),
                EntityRef::EventFrame(ev) => ev.icon.clone(),
                _ => None,
            };
            let icon_override = explicit_icon
                .or_else(|| icon_map_result.get(&common.id).cloned())
                .map(|n| self.resolve_entity_icon(&n));

            self.resolved_entities.push(ResolvedEntity {
                ci_id,
                int_id,
                identity: common.id.clone(),
                label,
                etype: common.r#type.clone(),
                etype_ref,
                x,
                y,
                attrs,
                grade_one,
                grade_two,
                grade_three,
                auto_shade,
                label_bg,
                label_fg,
                cards,
                semantic_guid,
                icon_override,
            });
        }
    }

    fn register_links(&mut self, data: &ChartData) {
        // Intensity styling needs the whole link set (domain spans all links);
        // its per-link (width, color) override is the only styling precomputed.
        let intensity: Vec<(Option<i64>, Option<u32>)> = self
            .settings
            .extra_cfg
            .styling
            .as_ref()
            .and_then(|s| s.links.as_ref())
            .and_then(|l| l.intensity.as_ref())
            .map(|ic| crate::transforms::intensity_overrides(&data.links, ic))
            .unwrap_or_default();
        // Parallel-link arc offsets, computed across the whole set up front.
        let spacing = self.settings.extra_cfg.link_arc_offset.unwrap_or(20);
        let auto_offsets = crate::transforms::compute_link_offsets(&data.links, spacing);
        for (li, l) in data.links.iter().enumerate() {
            let (int_width, int_color) = intensity.get(li).copied().unwrap_or((None, None));
            // Categorical strength is needed now (it feeds strength registration);
            // the colour/width it sets are recomputed lazily at emit.
            let style_strength = self.categorical_strength(l);
            if let Some(t) = &l.r#type {
                if !t.is_empty() {
                    self.register_link_type(t);
                }
            }
            self.ensure_strength(l.strength.as_deref().or(style_strength.as_deref()));
            // Connection style (multiplicity/fan_out/theme_wiring) — minted after
            // strength, before the chart-item id, deduped by style tuple.
            let conn_ref = self.register_connection(l);
            let ci_id = self.ids.next_id();
            // Register the link's attribute classes (mint ids); the resolved
            // attribute list is rebuilt lazily at emit, never stored here.
            let _ = self.resolve_attrs(&l.attributes);
            let semantic_guid = self.resolver.resolve_type_name(l.semantic_type.as_deref());
            let offset = l
                .offset
                .unwrap_or_else(|| auto_offsets.get(li).copied().unwrap_or(0));
            self.link_meta.push(LinkMeta {
                ci_id,
                semantic_guid,
                int_width,
                int_color,
                offset,
                conn_ref,
            });
        }
    }

    /// Register a deduped `<Connection>` for a link's connection style, returning
    /// its id (or `None` when the link carries no multiplicity/fan_out/theme_wiring).
    fn register_connection(&mut self, l: &crate::models::Link) -> Option<String> {
        let key = (
            l.multiplicity.clone().filter(|s| !s.is_empty()),
            l.fan_out,
            l.theme_wiring.clone().filter(|s| !s.is_empty()),
        );
        if key.0.is_none() && key.1.is_none() && key.2.is_none() {
            return None;
        }
        if let Some(id) = self.connections.get(&key) {
            return Some(id.clone());
        }
        let id = self.ids.next_id();
        self.connections.insert(key, id.clone());
        Some(id)
    }

    /// The categorical strength override for a link (only when the link left
    /// `strength` unset). Read-only.
    fn categorical_strength(&self, l: &crate::models::Link) -> Option<String> {
        if l.strength.is_some() {
            return None;
        }
        let cat = self
            .settings
            .extra_cfg
            .styling
            .as_ref()
            .and_then(|s| s.links.as_ref())
            .and_then(|c| c.categorical.as_ref())?;
        crate::transforms::categorical_style(&l.attributes, cat).and_then(|st| st.strength.clone())
    }

    /// Register an entity type as a collection entry iff it has config metadata.
    /// Referenced-only bare types get no entry (matching upstream).
    /// Convert + encode a batch of custom icons, recording their emitted names.
    fn prepare_custom_icons(
        &mut self,
        entries: &[crate::models::CustomIconEntry],
        kind: crate::custom_icons::IconKind,
    ) {
        for e in entries {
            if e.name.is_empty() {
                continue;
            }
            let prefix = e
                .prefix
                .clone()
                .unwrap_or_else(|| crate::custom_icons::EMITTED_PREFIX.to_string());
            let emitted = format!("{prefix}{}", e.name);
            let (data, datalength) = if let Some(d) = &e.data {
                (d.clone(), e.datalength.unwrap_or(0))
            } else if let Some(img) = &e.image {
                match crate::custom_icons::to_bmp(img) {
                    Ok(bmp) => crate::custom_icons::payload(&bmp),
                    Err(_) => continue, // skip an icon that fails to convert
                }
            } else {
                continue;
            };
            let map = match kind {
                crate::custom_icons::IconKind::Icon => &mut self.custom_entity_icon_names,
                crate::custom_icons::IconKind::Attribute => &mut self.custom_attribute_icon_names,
            };
            map.insert(e.name.clone(), emitted.clone());
            self.custom_images.push((
                crate::custom_icons::composite_key(&emitted, kind),
                datalength,
                data,
                emitted,
            ));
        }
    }

    /// Resolve a (possibly custom) entity-icon name to its emitted name, marking
    /// the icon as referenced.
    fn resolve_entity_icon(&mut self, name: &str) -> String {
        if let Some(em) = self.custom_entity_icon_names.get(name) {
            let em = em.clone();
            self.used_custom_images.insert(em.clone());
            em
        } else {
            name.to_string()
        }
    }

    fn resolve_attr_icon(&mut self, name: &str) -> String {
        if let Some(em) = self.custom_attribute_icon_names.get(name) {
            let em = em.clone();
            self.used_custom_images.insert(em.clone());
            em
        } else {
            name.to_string()
        }
    }

    /// Register a numeric geo attribute class (Latitude/Longitude) with its i2
    /// semantic property GUID. Returns the minted reference id.
    fn register_geo_ac(&mut self, name: &str) -> String {
        if let Some(e) = self.attribute_classes.get(name) {
            return e.id.clone();
        }
        let id = self.ids.next_id();
        let semantic_guid = self.resolver.resolve_property_name(Some(name));
        let cfg = AttributeClass {
            name: name.to_string(),
            r#type: Some(crate::enums::AttributeType::Number),
            show_value: Some(true),
            ..Default::default()
        };
        self.attribute_classes.insert(
            name.to_string(),
            AcEntry {
                id: id.clone(),
                type_token: "AttNumber".to_string(),
                cfg,
                semantic_guid,
            },
        );
        id
    }

    fn register_entity_type(&mut self, name: &str) -> Option<String> {
        if name.is_empty() {
            return None;
        }
        if let Some(e) = self.entity_types.get(name) {
            return Some(e.id.clone());
        }
        let mut cfg = self.cfg_entity_types.shift_remove(name)?;
        if let Some(icon) = cfg.icon_file.clone() {
            cfg.icon_file = Some(self.resolve_entity_icon(&icon));
        }
        let id = self.ids.next_id();
        let semantic_guid = self
            .resolver
            .resolve_type_name(cfg.semantic_type.as_deref());
        self.entity_types.insert(
            name.to_string(),
            EtEntry {
                id: id.clone(),
                cfg,
                semantic_guid,
            },
        );
        Some(id)
    }

    /// Link types are always registered (auto-created) on first reference.
    fn register_link_type(&mut self, name: &str) -> String {
        if let Some(e) = self.link_types.get(name) {
            return e.id.clone();
        }
        let cfg = self
            .cfg_link_types
            .shift_remove(name)
            .unwrap_or_else(|| LinkType {
                name: name.to_string(),
                ..Default::default()
            });
        let id = self.ids.next_id();
        let semantic_guid = self
            .resolver
            .resolve_type_name(cfg.semantic_type.as_deref());
        self.link_types.insert(
            name.to_string(),
            LtEntry {
                id: id.clone(),
                cfg,
                semantic_guid,
            },
        );
        id
    }

    /// Resolve a list of input cards plus any matching loose cards (routed by
    /// entity_id/link_id) into emit-ready [`ResolvedCard`]s.
    fn resolve_cards(
        &self,
        inline: &[crate::models::Card],
        loose: &[crate::models::Card],
        owner_id: Option<&str>,
        is_link: bool,
    ) -> Vec<crate::resolved::ResolvedCard> {
        let matched_loose = loose.iter().filter(|c| {
            let target = if is_link {
                c.link_id.as_deref()
            } else {
                c.entity_id.as_deref()
            };
            owner_id.is_some() && target == owner_id
        });
        inline
            .iter()
            .chain(matched_loose)
            .map(|c| self.resolve_card(c))
            .collect()
    }

    fn resolve_card(&self, c: &crate::models::Card) -> crate::resolved::ResolvedCard {
        let (datetime, date_set, time_set) =
            match crate::datetime::build_datetime(c.date.as_deref(), c.time.as_deref()) {
                Some((dt, ds, ts)) => (Some(dt), ds, ts),
                None => (None, false, false),
            };
        crate::resolved::ResolvedCard {
            summary: c.summary.clone(),
            datetime,
            date_set,
            time_set,
            grade_one: crate::transforms::resolve_grade_with_default(
                c.grade_one.as_ref(),
                &self.grades_one,
            ),
            grade_two: crate::transforms::resolve_grade_with_default(
                c.grade_two.as_ref(),
                &self.grades_two,
            ),
            grade_three: crate::transforms::resolve_grade_with_default(
                c.grade_three.as_ref(),
                &self.grades_three,
            ),
            source_ref: c.source_ref.clone(),
            source_type: c.source_type.clone(),
            description: c.description.clone(),
            datetime_description: c.datetime_description.clone(),
            timezone_id: c.timezone.as_ref().map(|t| t.id),
            timezone_name: c.timezone.as_ref().map(|t| t.name.clone()),
        }
    }

    fn default_strength(&self) -> String {
        self.strength_default
            .clone()
            .unwrap_or_else(|| "Default".to_string())
    }

    fn ensure_strength(&mut self, name: Option<&str>) -> String {
        let default = self.default_strength();
        let name = name.unwrap_or(&default).to_string();
        if !self.strengths.contains_key(&name) {
            let id = self.ids.next_id();
            self.strengths.insert(
                name.clone(),
                StrengthEntry {
                    id,
                    dot_token: "DotStyleSolid".to_string(),
                },
            );
        }
        name
    }

    fn resolve_attrs(&mut self, attrs: &IndexMap<String, AttrValue>) -> Vec<ResolvedAttr> {
        let mut out = Vec::new();
        for (name, value) in attrs {
            let reference = self.register_attribute_class(name, value);
            out.push(ResolvedAttr {
                class: name.clone(),
                reference,
                value: Some(value.render()),
            });
        }
        out
    }

    /// Build resolved attributes by looking up already-registered class ids
    /// (read-only — used during the lazy link emit pass).
    fn lookup_attrs(&self, attrs: &IndexMap<String, AttrValue>) -> Vec<ResolvedAttr> {
        attrs
            .iter()
            .map(|(name, value)| ResolvedAttr {
                class: name.clone(),
                reference: self
                    .attribute_classes
                    .get(name)
                    .map(|e| e.id.clone())
                    .unwrap_or_default(),
                value: Some(value.render()),
            })
            .collect()
    }

    fn register_attribute_class(&mut self, name: &str, sample: &AttrValue) -> String {
        if let Some(e) = self.attribute_classes.get(name) {
            return e.id.clone();
        }
        let mut cfg = self
            .cfg_attribute_classes
            .shift_remove(name)
            .unwrap_or_else(|| AttributeClass {
                name: name.to_string(),
                ..Default::default()
            });
        if let Some(icon) = cfg.icon_file.clone() {
            cfg.icon_file = Some(self.resolve_attr_icon(&icon));
        }
        let type_token = match &cfg.r#type {
            Some(t) => attribute_type_token(&serde_to_str(t)).to_string(),
            None => sample.infer_type().anb_token().to_string(),
        };
        let id = self.ids.next_id();
        let semantic_guid = self
            .resolver
            .resolve_property_name(cfg.semantic_type.as_deref());
        self.attribute_classes.insert(
            name.to_string(),
            AcEntry {
                id: id.clone(),
                type_token,
                cfg,
                semantic_guid,
            },
        );
        id
    }

    /// After entities/links, promote config types never referenced so they
    /// still appear in their collections.
    fn register_remaining_config_types(&mut self) {
        let remaining_et: Vec<EntityType> = self.cfg_entity_types.values().cloned().collect();
        for mut cfg in remaining_et {
            if let Some(icon) = cfg.icon_file.clone() {
                cfg.icon_file = Some(self.resolve_entity_icon(&icon));
            }
            let id = self.ids.next_id();
            let semantic_guid = self
                .resolver
                .resolve_type_name(cfg.semantic_type.as_deref());
            self.entity_types.insert(
                cfg.name.clone(),
                EtEntry {
                    id,
                    cfg,
                    semantic_guid,
                },
            );
        }
        self.cfg_entity_types.clear();

        let remaining_lt: Vec<LinkType> = self.cfg_link_types.values().cloned().collect();
        for cfg in remaining_lt {
            let id = self.ids.next_id();
            let semantic_guid = self
                .resolver
                .resolve_type_name(cfg.semantic_type.as_deref());
            self.link_types.insert(
                cfg.name.clone(),
                LtEntry {
                    id,
                    cfg,
                    semantic_guid,
                },
            );
        }
        self.cfg_link_types.clear();

        let remaining_ac: Vec<AttributeClass> =
            self.cfg_attribute_classes.values().cloned().collect();
        for cfg in remaining_ac {
            let type_token = cfg
                .r#type
                .as_ref()
                .map(|t| attribute_type_token(&serde_to_str(t)).to_string())
                .unwrap_or_else(|| "AttText".to_string());
            let id = self.ids.next_id();
            let semantic_guid = self
                .resolver
                .resolve_property_name(cfg.semantic_type.as_deref());
            self.attribute_classes.insert(
                cfg.name.clone(),
                AcEntry {
                    id,
                    type_token,
                    cfg,
                    semantic_guid,
                },
            );
        }
        self.cfg_attribute_classes.clear();
    }

    /// Expand `display_attribute` (synthesized sibling AC) and `display_label`
    /// (templated label) entries over the resolved items.
    fn expand_displays(&mut self) {
        let da = self.settings.extra_cfg.display_attribute.clone();
        let dl = self.settings.extra_cfg.display_label.clone();

        for disp in &da {
            let (Some(attr_name), Some(template)) =
                (disp.attribute_name.clone(), disp.template.clone())
            else {
                continue;
            };
            let metas = crate::transforms::source_metas(&disp.sources);
            // Register (or reuse) the visible text sibling attribute class.
            let ref_id = match self.attribute_classes.get(&attr_name) {
                Some(e) => e.id.clone(),
                None => {
                    let id = self.ids.next_id();
                    let cfg = AttributeClass {
                        name: attr_name.clone(),
                        r#type: Some(crate::enums::AttributeType::Text),
                        visible: Some(true),
                        show_value: Some(true),
                        ..Default::default()
                    };
                    self.attribute_classes.insert(
                        attr_name.clone(),
                        AcEntry {
                            id: id.clone(),
                            type_token: "AttText".to_string(),
                            cfg,
                            semantic_guid: None,
                        },
                    );
                    id
                }
            };
            let kind = disp.kind.as_deref();
            let tfilter = disp.r#type.as_deref();
            if crate::transforms::display_kind_matches(kind, false) {
                for re in &mut self.resolved_entities {
                    if tfilter.is_some_and(|t| t != re.etype) {
                        continue;
                    }
                    let lookup = attr_lookup(&re.attrs);
                    if let Some(r) = crate::transforms::render_display(
                        &lookup,
                        &template,
                        &metas,
                        disp.decimal_separator.as_deref().unwrap_or("."),
                        disp.thousand_separator.as_deref().unwrap_or(","),
                    ) {
                        re.attrs.push(ResolvedAttr {
                            class: attr_name.clone(),
                            reference: ref_id.clone(),
                            value: Some(r),
                        });
                    }
                }
            }
            // Link display attributes are synthesized lazily during emit
            // (see `apply_link_display`); only the sibling class is registered here.
        }

        for disp in &dl {
            let Some(template) = disp.template.clone() else {
                continue;
            };
            let metas = crate::transforms::source_metas(&disp.sources);
            let override_existing = disp.override_existing.unwrap_or(false);
            let kind = disp.kind.as_deref();
            let tfilter = disp.r#type.as_deref();
            if crate::transforms::display_kind_matches(kind, false) {
                for re in &mut self.resolved_entities {
                    if tfilter.is_some_and(|t| t != re.etype) {
                        continue;
                    }
                    let explicit = !re.label.is_empty() && re.label != re.identity;
                    if explicit && !override_existing {
                        continue;
                    }
                    let lookup = attr_lookup(&re.attrs);
                    if let Some(r) = crate::transforms::render_display(
                        &lookup,
                        &template,
                        &metas,
                        disp.decimal_separator.as_deref().unwrap_or("."),
                        disp.thousand_separator.as_deref().unwrap_or(","),
                    ) {
                        re.label = r;
                    }
                }
            }
            // Link display labels are applied lazily during emit
            // (see `apply_link_display`).
        }
    }

    // ── Emit pass ─────────────────────────────────────────────────────────────

    fn emit_into(&self, w: &mut Writer, data: &ChartData) {
        w.declaration();
        w.comment(&format!("Built with anxwritter {VERSION} — {REPO_URL}"));
        w.open("Chart", &self.chart_attrs());
        w.empty(
            "ApplicationVersion",
            &[
                ("Major", "9".into()),
                ("Minor", "0".into()),
                ("Point", "0".into()),
                ("Build", "0".into()),
            ],
        );
        self.emit_library_catalogue(w);
        self.emit_custom_images(w);
        self.emit_strengths(w);
        self.emit_grades(w);
        self.emit_attribute_classes(w);
        self.emit_entity_types(w);
        self.emit_link_types(w);
        self.emit_datetime_formats(w);
        self.emit_summary(w);
        self.emit_chart_items(w, data);
        self.emit_connections(w);
        self.emit_palette(w);
        self.emit_legend(w);
        w.close("Chart");
    }

    fn emit_custom_images(&self, w: &mut Writer) {
        let all = self.custom_icons_include == "all";
        let kept: Vec<&(String, u32, String, String)> = self
            .custom_images
            .iter()
            .filter(|(_, _, _, em)| all || self.used_custom_images.contains(em))
            .collect();
        if kept.is_empty() {
            return;
        }
        w.open("CustomImageCollection", &[]);
        for (id, dl, data, _) in kept {
            w.empty(
                "CustomImage",
                &[
                    ("Id", id.clone()),
                    ("DataLength", dl.to_string()),
                    ("Data", data.clone()),
                ],
            );
        }
        w.close("CustomImageCollection");
    }

    fn emit_library_catalogue(&self, w: &mut Writer) {
        let Some((types, props)) = self.resolver.build_catalogue() else {
            return;
        };
        w.open(
            "lcx:LibraryCatalogue",
            &[
                ("VersionMajor", crate::interop::LCX_VERSION_MAJOR.into()),
                ("VersionMinor", crate::interop::LCX_VERSION_MINOR.into()),
                ("VersionRelease", crate::interop::LCX_VERSION_RELEASE.into()),
                ("VersionBuild", crate::interop::LCX_VERSION_BUILD.into()),
                ("LocaleHex", crate::interop::LCX_LOCALE_HEX.into()),
            ],
        );
        for t in &types {
            emit_lcx_type(w, "lcx:Type", "tGUID", "kindOf", "TypeName", t);
        }
        for p in &props {
            emit_lcx_type(
                w,
                "lcx:Property",
                "pGUID",
                "baseProperty",
                "PropertyName",
                p,
            );
        }
        w.close("lcx:LibraryCatalogue");
    }

    fn chart_attrs(&self) -> Vec<Attr<'static>> {
        let mut a: Vec<Attr<'static>> = Vec::new();
        // The lcx namespace is declared first when a LibraryCatalogue is emitted.
        if self.resolver.build_catalogue().is_some() {
            a.push(("xmlns:lcx", crate::interop::LCX_NS.to_string()));
        }
        let s = &self.settings;
        let mut push_bool = |name: &'static str, v: Option<bool>| {
            if let Some(v) = v {
                a.push((name, b(v)));
            }
        };
        push_bool("IsBackColourFilled", s.chart.bg_filled);
        push_bool("Rigorous", s.chart.rigorous);
        push_bool("IdReferenceLinking", s.chart.id_reference_linking);
        push_bool("SnapToGrid", s.grid.snap);
        push_bool("GridVisibleOnAllViews", s.grid.visible);
        push_bool("ShowAllFlag", s.view.show_all);
        push_bool("ShowPages", s.view.show_pages_boundaries);
        push_bool("CoverSheetShowOnOpen", s.view.cover_sheet_on_open);
        push_bool("TimeBarVisible", s.view.time_bar);
        push_bool("BlankLinkLabels", s.links_cfg.blank_labels);
        push_bool("LabelSumNumericLinks", s.links_cfg.sum_numeric_labels);
        push_bool("UseLocalTimeZone", s.time.local_tz);
        push_bool("HideMatchingTimeZoneFormat", s.time.hide_matching_tz_format);
        if let Some(c) = resolve_color(&s.chart.bg_color) {
            a.push(("BackColour", c.to_string()));
        }
        if let Some(v) = s.grid.width {
            a.push(("GridWidthSize", v.to_string()));
        }
        if let Some(v) = s.grid.height {
            a.push(("GridHeightSize", v.to_string()));
        }
        a
    }

    fn emit_strengths(&self, w: &mut Writer) {
        w.open("StrengthCollection", &[]);
        for (name, e) in &self.strengths {
            w.empty(
                "Strength",
                &[
                    ("Id", e.id.clone()),
                    ("Name", name.clone()),
                    ("DotStyle", e.dot_token.clone()),
                ],
            );
        }
        w.close("StrengthCollection");
    }

    fn emit_grades(&self, w: &mut Writer) {
        // Each grade dimension and source hints become a <StringCollection>.
        for (dim, (tag, gc)) in [
            ("GradeOne", &self.grades_one),
            ("GradeTwo", &self.grades_two),
            ("GradeThree", &self.grades_three),
        ]
        .into_iter()
        .enumerate()
        {
            if gc.items.is_empty() {
                continue;
            }
            w.open(tag, &[]);
            w.open("StringCollection", &[]);
            for (i, item) in gc.items.iter().enumerate() {
                w.empty(
                    "String",
                    &[
                        ("Id", self.grade_ids[dim][i].clone()),
                        ("Text", item.clone()),
                    ],
                );
            }
            w.close("StringCollection");
            w.close(tag);
        }
        if !self.source_types.is_empty() {
            w.open("SourceHints", &[]);
            w.open("StringCollection", &[]);
            for (i, s) in self.source_types.iter().enumerate() {
                w.empty(
                    "String",
                    &[("Id", self.source_ids[i].clone()), ("Text", s.clone())],
                );
            }
            w.close("StringCollection");
            w.close("SourceHints");
        }
    }

    fn emit_attribute_classes(&self, w: &mut Writer) {
        if self.attribute_classes.is_empty() {
            return;
        }
        w.open("AttributeClassCollection", &[]);
        for (name, e) in &self.attribute_classes {
            let mut attrs: Vec<Attr> = vec![
                ("Id", e.id.clone()),
                ("Name", name.clone()),
                ("Type", e.type_token.clone()),
                // Auto-created defaults — always emitted.
                ("IsUser", b(e.cfg.is_user.unwrap_or(true))),
                ("UserCanAdd", b(e.cfg.user_can_add.unwrap_or(true))),
                ("UserCanRemove", b(e.cfg.user_can_remove.unwrap_or(true))),
                ("ShowValue", b(e.cfg.show_value.unwrap_or(true))),
            ];
            if let Some(icon) = &e.cfg.icon_file {
                attrs.push(("IconFile", icon.clone()));
            }
            if let Some(p) = &e.cfg.prefix {
                attrs.push(("Prefix", p.clone()));
                attrs.push(("ShowPrefix", b(!p.is_empty())));
            }
            if let Some(suf) = &e.cfg.suffix {
                attrs.push(("Suffix", suf.clone()));
                attrs.push(("ShowSuffix", b(!suf.is_empty())));
            }
            if let Some(v) = e.cfg.show_class_name {
                attrs.push(("ShowClassName", b(v)));
            }
            if let Some(v) = e.cfg.show_symbol {
                attrs.push(("ShowSymbol", b(v)));
            }
            if let Some(v) = e.cfg.decimal_places {
                attrs.push(("DecimalPlaces", v.to_string()));
            }
            if let Some(v) = e.cfg.visible {
                attrs.push(("Visible", b(v)));
            }
            if let Some(v) = e.cfg.show_if_set {
                attrs.push(("ShowIfSet", b(v)));
            }
            if let Some(m) = &e.cfg.merge_behaviour {
                attrs.push((
                    "MergeBehaviour",
                    merge_behaviour_token(&serde_to_str(m)).into(),
                ));
            }
            if let Some(m) = &e.cfg.paste_behaviour {
                attrs.push((
                    "PasteBehaviour",
                    merge_behaviour_token(&serde_to_str(m)).into(),
                ));
            }
            if let Some(sg) = &e.semantic_guid {
                attrs.push(("SemanticTypeGuid", sg.clone()));
            }
            w.empty("AttributeClass", &attrs);
        }
        w.close("AttributeClassCollection");
    }

    fn emit_entity_types(&self, w: &mut Writer) {
        if self.entity_types.is_empty() {
            return;
        }
        w.open("EntityTypeCollection", &[]);
        for (name, e) in &self.entity_types {
            let mut attrs: Vec<Attr> = vec![("Id", e.id.clone()), ("Name", name.clone())];
            if let Some(rep) = &e.cfg.representation {
                attrs.push(("PreferredRepresentation", representation_token(rep).into()));
            }
            if let Some(icon) = &e.cfg.icon_file {
                attrs.push(("IconFile", icon.clone()));
            }
            if let Some(c) = resolve_color(&e.cfg.color) {
                attrs.push(("Colour", c.to_string()));
            }
            if let Some(c) = resolve_color(&e.cfg.shade_color) {
                attrs.push(("IconShadingColour", c.to_string()));
            }
            if let Some(sg) = &e.semantic_guid {
                attrs.push(("SemanticTypeGuid", sg.clone()));
            }
            w.empty("EntityType", &attrs);
        }
        w.close("EntityTypeCollection");
    }

    fn emit_link_types(&self, w: &mut Writer) {
        if self.link_types.is_empty() {
            return;
        }
        w.open("LinkTypeCollection", &[]);
        for (name, e) in &self.link_types {
            let mut attrs: Vec<Attr> = vec![("Id", e.id.clone()), ("Name", name.clone())];
            if let Some(c) = resolve_color(&e.cfg.color) {
                attrs.push(("Colour", c.to_string()));
            }
            if let Some(sg) = &e.semantic_guid {
                attrs.push(("SemanticTypeGuid", sg.clone()));
            }
            w.empty("LinkType", &attrs);
        }
        w.close("LinkTypeCollection");
    }

    fn emit_datetime_formats(&self, w: &mut Writer) {
        if self.datetime_formats.is_empty() {
            return;
        }
        w.open("DateTimeFormatCollection", &[]);
        for (id, name, fmt) in &self.datetime_formats {
            let mut attrs: Vec<Attr> = vec![("Id", id.clone()), ("Name", name.clone())];
            if !fmt.is_empty() {
                attrs.push(("Format", fmt.clone()));
            }
            w.empty("DateTimeFormat", &attrs);
        }
        w.close("DateTimeFormatCollection");
    }

    fn emit_summary(&self, w: &mut Writer) {
        let s = &self.summary;
        let fields: [(&str, &Option<String>); 7] = [
            ("SummaryFieldTitle", &s.title),
            ("SummaryFieldSubject", &s.subject),
            ("SummaryFieldKeywords", &s.keywords),
            ("SummaryFieldCategory", &s.category),
            ("SummaryFieldComments", &s.comments),
            ("SummaryFieldAuthor", &s.author),
            ("SummaryFieldTemplate", &s.template),
        ];
        let has_fields = fields
            .iter()
            .any(|(_, v)| v.as_deref().is_some_and(|x| !x.is_empty()));
        if !has_fields && s.custom_properties.is_empty() {
            return;
        }
        w.open("Summary", &[]);
        if has_fields {
            w.open("FieldCollection", &[]);
            for (ty, val) in fields {
                if let Some(v) = val {
                    if !v.is_empty() {
                        w.empty("Field", &[("Type", ty.into()), ("Field", v.clone())]);
                    }
                }
            }
            w.close("FieldCollection");
        }
        if !s.custom_properties.is_empty() {
            w.open("CustomPropertyCollection", &[]);
            for cp in &s.custom_properties {
                w.empty(
                    "CustomProperty",
                    &[
                        ("Name", cp.name.clone()),
                        ("Type", "String".into()),
                        ("Value", cp.value.clone()),
                    ],
                );
            }
            w.close("CustomPropertyCollection");
        }
        // Origin — CreatedDate is the one volatile attribute (blanked in digests).
        let created = s
            .created
            .clone()
            .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string());
        w.empty(
            "Origin",
            &[
                ("CreatedDate", created),
                ("EditTime", s.edit_time.unwrap_or(0).to_string()),
                ("RevisionNumber", s.revision.unwrap_or(1).to_string()),
            ],
        );
        w.close("Summary");
    }

    fn emit_legend(&self, w: &mut Writer) {
        let lc = &self.settings.legend_cfg;
        let font = &lc.font;
        let font_ov = font.name.is_some()
            || font.size.is_some()
            || font.color.is_some()
            || font.bg_color.is_some()
            || font.bold.is_some()
            || font.italic.is_some()
            || font.strikeout.is_some()
            || font.underline.is_some();
        let has_attrs = lc.arrange.is_some()
            || lc.valign.is_some()
            || lc.halign.is_some()
            || lc.x.is_some()
            || lc.y.is_some()
            || lc.show.is_some();
        // Emitted only with content (attrs, a font override, or items) — matches
        // Python's `_has_legend`.
        if !has_attrs && !font_ov && self.legend_items.is_empty() {
            return;
        }
        let mut attrs: Vec<Attr> = Vec::new();
        // `Arrange` defaults to `LegendArrangementWide`.
        attrs.push((
            "Arrange",
            legend_arrange_token(lc.arrange.as_deref().unwrap_or("wide")),
        ));
        if let Some(x) = lc.x {
            attrs.push(("X", x.to_string()));
        }
        if let Some(y) = lc.y {
            attrs.push(("Y", y.to_string()));
        }
        if let Some(s) = lc.show {
            attrs.push(("Shown", b(s)));
        }
        w.open("LegendDefinition", &attrs);
        if font_ov {
            emit_font_full(w, font, 10);
        }
        for li in &self.legend_items {
            let mut a: Vec<Attr> = vec![
                (
                    "Type",
                    legend_item_type_token(li.item_type.as_deref().unwrap_or("font")),
                ),
                ("Label", li.name.clone()),
            ];
            if let Some(img) = &li.image_name {
                a.push(("ImageName", img.clone()));
            }
            if let Some(c) = resolve_color(&li.color) {
                a.push(("Colour", c.to_string()));
            }
            if let Some(lw) = li.line_width {
                a.push(("LineWidth", lw.to_string()));
            }
            if let Some(ar) = li.arrows.as_deref().and_then(arrow_token) {
                a.push(("Arrows", ar.into()));
            }
            if let Some(ds) = &li.dash_style {
                a.push(("DashStyle", dot_style_token(ds).into()));
            }
            if let Some(c) = resolve_color(&li.shade_color) {
                a.push(("ShadeColour", c.to_string()));
            }
            w.empty("LegendItem", &a);
        }
        w.close("LegendDefinition");
    }

    fn emit_chart_items(&self, w: &mut Writer, data: &ChartData) {
        w.open("ChartItemCollection", &[]);
        // Re-borrow entities in the same order resolution used and zip with the
        // resolved metadata — the source entity is never cloned (see EntityRef).
        for (e, er) in self.resolved_entities.iter().zip(collect_entity_refs(data)) {
            self.emit_entity_item(w, e, er);
        }
        // Links are resolved lazily — one at a time, borrowed from the input —
        // so the full resolved-link set is never held in memory.
        for (i, l) in data.links.iter().enumerate() {
            self.emit_link_item(w, i, l, data);
        }
        w.close("ChartItemCollection");
    }

    fn emit_entity_item(&self, w: &mut Writer, e: &ResolvedEntity, er: EntityRef) {
        let c = er.common();
        let mut ci_attrs: Vec<Attr> = vec![("Id", e.ci_id.clone()), ("Label", e.label.clone())];
        ci_attrs.extend(meta_attrs(
            c.description.as_deref(),
            c.date.as_deref(),
            c.time.as_deref(),
            c.datetime_description.as_deref(),
            c.ordered,
            c.source_ref.as_deref(),
            c.source_type.as_deref(),
        ));
        push_grades(&mut ci_attrs, e.grade_one, e.grade_two, e.grade_three);
        // XPosition is emitted only for a placed entity (origin = unplaced),
        // matching Python's `if re.x or re.y`.
        if e.x != 0 || e.y != 0 {
            ci_attrs.push(("XPosition", e.x.to_string()));
        }
        w.open("ChartItem", &ci_attrs);
        w.open(
            "End",
            &[
                ("X", e.x.to_string()),
                ("Y", e.y.to_string()),
                ("Z", "0".into()),
            ],
        );
        let mut ent_attrs: Vec<Attr> = vec![
            ("EntityId", e.int_id.to_string()),
            ("Identity", e.identity.clone()),
            ("LabelIsIdentity", b(e.label == e.identity)),
        ];
        if let Some(sg) = &e.semantic_guid {
            ent_attrs.push(("SemanticTypeGuid", sg.clone()));
        }
        w.open("Entity", &ent_attrs);
        self.emit_representation(w, e, er);
        emit_cards(w, &e.cards);
        w.close("Entity");
        w.close("End");
        self.emit_attribute_collection(w, &e.attrs);
        let c = er.common();
        self.emit_ci_style(
            w,
            &c.label_font,
            &c.show,
            c.sub_text_width,
            c.use_sub_text_width,
            c.show_datetime_description,
            e.label_bg,
            e.label_fg,
        );
        emit_timezone(w, c.timezone.as_ref());
        w.close("ChartItem");
    }

    /// Emit `<CIStyle>` (label font + sub-item visibility) when any of its
    /// inputs are set. The label `<Font>` merges explicit `label_font` fields
    /// with auto-colour foreground/background.
    #[allow(clippy::too_many_arguments)]
    fn emit_ci_style(
        &self,
        w: &mut Writer,
        font: &crate::models::Font,
        show: &crate::models::Show,
        sub_text_width: Option<f64>,
        use_sub_text_width: Option<bool>,
        show_datetime_description: Option<bool>,
        auto_bg: Option<u32>,
        auto_fg: Option<u32>,
    ) {
        let font_colour = resolve_color(&font.color).or(auto_fg);
        let back_colour = resolve_color(&font.bg_color).or(auto_bg);
        let has_font = font_colour.is_some()
            || back_colour.is_some()
            || font.name.is_some()
            || font.size.is_some()
            || font.bold.is_some()
            || font.italic.is_some()
            || font.strikeout.is_some()
            || font.underline.is_some();
        let show_fields: [(&str, Option<bool>, bool); 7] = [
            ("SubItemDescription", show.description, false),
            ("SubItemGrades", show.grades, false),
            ("SubItemLabel", show.label, true),
            ("SubItemPin", show.pin, false),
            ("SubItemSourceReference", show.source_ref, false),
            ("SubItemSourceType", show.source_type, false),
            ("SubItemDateTime", show.date, false),
        ];
        let has_show = show_fields.iter().any(|(_, v, _)| v.is_some());

        let mut ci_attrs: Vec<Attr> = Vec::new();
        if let Some(v) = sub_text_width {
            ci_attrs.push(("SubTextWidth", format_f64(v)));
        }
        if let Some(v) = use_sub_text_width {
            ci_attrs.push(("UseSubTextWidth", b(v)));
        }
        if let Some(v) = show_datetime_description {
            ci_attrs.push(("ShowDateTimeDescription", b(v)));
        }
        if !has_font && !has_show && ci_attrs.is_empty() {
            return;
        }
        w.open("CIStyle", &ci_attrs);
        if has_font {
            let mut fa: Vec<Attr> = Vec::new();
            if let Some(c) = font_colour {
                fa.push(("FontColour", c.to_string()));
            }
            if let Some(c) = back_colour {
                fa.push(("BackColour", c.to_string()));
            }
            if let Some(n) = &font.name {
                fa.push(("FaceName", n.clone()));
            }
            if let Some(s) = font.size {
                fa.push(("PointSize", s.to_string()));
            }
            if let Some(v) = font.bold {
                fa.push(("Bold", b(v)));
            }
            if let Some(v) = font.italic {
                fa.push(("Italic", b(v)));
            }
            if let Some(v) = font.strikeout {
                fa.push(("Strikeout", b(v)));
            }
            if let Some(v) = font.underline {
                fa.push(("Underline", b(v)));
            }
            w.empty("Font", &fa);
        }
        if has_show {
            w.open("SubItemCollection", &[]);
            for (ty, val, default) in show_fields {
                w.empty(
                    "SubItem",
                    &[
                        ("Type", ty.to_string()),
                        ("Visible", b(val.unwrap_or(default))),
                    ],
                );
            }
            w.close("SubItemCollection");
        }
        w.close("CIStyle");
    }

    fn emit_representation(&self, w: &mut Writer, e: &ResolvedEntity, er: EntityRef) {
        let type_ref = e.etype_ref.clone();
        let strength = self.default_strength();
        match er {
            EntityRef::Icon(icon) => {
                let mut icon_attrs: Vec<Attr> = Vec::new();
                if let Some(tx) = icon.text_x {
                    icon_attrs.push(("TextX", tx.to_string()));
                }
                if let Some(ty) = icon.text_y {
                    icon_attrs.push(("TextY", ty.to_string()));
                }
                w.open("Icon", &icon_attrs);
                let mut style: Vec<Attr> = vec![("Type", e.etype.clone())];
                if let Some(r) = &type_ref {
                    style.push(("EntityTypeReference", r.clone()));
                }
                push_icon_override(&mut style, &e.icon_override);
                if let Some(enl) = &icon.enlargement {
                    style.push(("Enlargement", enlargement_token(enl)));
                }
                if let Some(c) = resolve_color(&icon.color)
                    .or(e.auto_shade)
                    .filter(|c| *c != 0)
                {
                    style.push(("IconShadingColour", c.to_string()));
                }
                w.empty("IconStyle", &style);
                w.close("Icon");
            }
            EntityRef::Box(bx) => {
                w.open("Box", &box_dims(bx.width, bx.height, bx.depth));
                let mut style: Vec<Attr> =
                    vec![("Strength", strength.clone()), ("Type", e.etype.clone())];
                push_type_ref(&mut style, &type_ref);
                push_fill(
                    &mut style,
                    resolve_color(&bx.bg_color),
                    bx.filled,
                    bx.line_width,
                );
                w.empty("BoxStyle", &style);
                w.close("Box");
            }
            EntityRef::Circle(c) => {
                w.open("Circle", &[]);
                let mut style: Vec<Attr> =
                    vec![("Strength", strength.clone()), ("Type", e.etype.clone())];
                push_type_ref(&mut style, &type_ref);
                push_fill(
                    &mut style,
                    resolve_color(&c.bg_color),
                    c.filled,
                    c.line_width,
                );
                if let Some(d) = c.diameter {
                    style.push(("Diameter", (d as f64 / 100.0).to_string()));
                }
                if let Some(v) = c.autosize {
                    style.push(("Autosize", b(v)));
                }
                w.empty("CircleStyle", &style);
                w.close("Circle");
            }
            EntityRef::TextBlock(t) => {
                w.open("TextBlock", &[]);
                let mut style: Vec<Attr> =
                    vec![("Strength", strength.clone()), ("Type", e.etype.clone())];
                push_type_ref(&mut style, &type_ref);
                if let Some(al) = &t.alignment {
                    style.push(("Alignment", text_align_token(al)));
                }
                push_fill(
                    &mut style,
                    resolve_color(&t.bg_color),
                    t.filled,
                    t.line_width,
                );
                if let Some(c) = resolve_color(&t.line_color) {
                    style.push(("LineColour", c.to_string()));
                }
                push_wh(&mut style, t.width, t.height);
                w.empty("TextBlockStyle", &style);
                w.close("TextBlock");
            }
            EntityRef::Label(l) => {
                // Label renders as a TextBlock with an invisible (bg-coloured)
                // border and Filled=false.
                let bg = resolve_color(&self.settings.chart.bg_color).unwrap_or(16777215);
                w.open("TextBlock", &[]);
                let mut style: Vec<Attr> = vec![
                    ("Strength", strength.clone()),
                    ("Type", e.etype.clone()),
                    ("BackColour", bg.to_string()),
                    ("Filled", "false".into()),
                    ("LineColour", bg.to_string()),
                ];
                push_type_ref(&mut style, &type_ref);
                if let Some(al) = &l.alignment {
                    style.push(("Alignment", text_align_token(al)));
                }
                push_wh(&mut style, l.width, l.height);
                w.empty("TextBlockStyle", &style);
                w.close("TextBlock");
            }
            EntityRef::EventFrame(ev) => {
                w.open("Event", &[]);
                let mut style: Vec<Attr> =
                    vec![("Strength", strength.clone()), ("Type", e.etype.clone())];
                push_type_ref(&mut style, &type_ref);
                push_fill(
                    &mut style,
                    resolve_color(&ev.bg_color),
                    ev.filled,
                    ev.line_width,
                );
                if let Some(c) = resolve_color(&ev.shade_color)
                    .or(e.auto_shade)
                    .filter(|c| *c != 0)
                {
                    style.push(("IconShadingColour", c.to_string()));
                }
                if let Some(c) = resolve_color(&ev.line_color).or(e.auto_shade) {
                    style.push(("LineColour", c.to_string()));
                }
                push_icon_override(&mut style, &e.icon_override);
                w.empty("EventStyle", &style);
                w.close("Event");
            }
            EntityRef::ThemeLine(th) => {
                w.open("Theme", &[]);
                let mut style: Vec<Attr> =
                    vec![("Strength", strength.clone()), ("Type", e.etype.clone())];
                push_type_ref(&mut style, &type_ref);
                if let Some(lw) = th.line_width {
                    style.push(("LineWidth", lw.to_string()));
                }
                if let Some(c) = resolve_color(&th.shade_color)
                    .or(e.auto_shade)
                    .filter(|c| *c != 0)
                {
                    style.push(("IconShadingColour", c.to_string()));
                }
                if let Some(c) = resolve_color(&th.line_color).or(e.auto_shade) {
                    style.push(("LineColour", c.to_string()));
                }
                push_icon_override(&mut style, &e.icon_override);
                w.empty("ThemeStyle", &style);
                w.close("Theme");
            }
        }
    }

    fn emit_attribute_collection(&self, w: &mut Writer, attrs: &[ResolvedAttr]) {
        if attrs.iter().all(|a| a.value.is_none()) {
            return;
        }
        w.open("AttributeCollection", &[]);
        for a in attrs {
            let mut at: Vec<Attr> = vec![
                ("AttributeClass", a.class.clone()),
                ("AttributeClassReference", a.reference.clone()),
            ];
            if let Some(v) = &a.value {
                at.push(("Value", v.clone()));
            }
            w.empty("Attribute", &at);
        }
        w.close("AttributeCollection");
    }

    fn emit_link_item(&self, w: &mut Writer, i: usize, l: &crate::models::Link, data: &ChartData) {
        let meta = &self.link_meta[i];
        // ── Re-derive resolved fields from the borrowed link (lazy) ──
        let end1 = self.entity_int_by_id.get(&l.from_id).copied().unwrap_or(0);
        let end2 = self.entity_int_by_id.get(&l.to_id).copied().unwrap_or(0);
        let (mut style_color, mut style_width, mut style_strength) = (None, None, None);
        if let Some(cat) = self
            .settings
            .extra_cfg
            .styling
            .as_ref()
            .and_then(|s| s.links.as_ref())
            .and_then(|c| c.categorical.as_ref())
        {
            if let Some(st) = crate::transforms::categorical_style(&l.attributes, cat) {
                if l.line_color.is_none() {
                    style_color = resolve_color(&st.line_color);
                }
                if l.line_width.is_none() {
                    style_width = st.line_width.map(|w| w.max(0));
                }
                if l.strength.is_none() {
                    style_strength = st.strength.clone();
                }
            }
        }
        let matched_color = if self.settings.extra_cfg.link_match_entity_color == Some(true)
            && l.line_color.is_none()
        {
            self.entity_color_map.get(&l.to_id).copied()
        } else {
            None
        };
        let line_width = l.line_width.or(style_width).or(meta.int_width);
        let line_color = resolve_color(&l.line_color)
            .or(style_color)
            .or(meta.int_color)
            .or(matched_color);
        let strength = l
            .strength
            .clone()
            .or(style_strength)
            .unwrap_or_else(|| self.default_strength());
        let arrow = l.arrow.as_deref().and_then(arrow_token);
        let ltype = l.r#type.as_deref().filter(|t| !t.is_empty());
        let ltype_ref = ltype.and_then(|t| self.link_types.get(t).map(|e| e.id.clone()));
        let grade_one =
            crate::transforms::resolve_grade_with_default(l.grade_one.as_ref(), &self.grades_one);
        let grade_two =
            crate::transforms::resolve_grade_with_default(l.grade_two.as_ref(), &self.grades_two);
        let grade_three = crate::transforms::resolve_grade_with_default(
            l.grade_three.as_ref(),
            &self.grades_three,
        );
        let cards = self.resolve_cards(&l.cards, &data.loose_cards, l.link_id.as_deref(), true);
        let mut attrs = self.lookup_attrs(&l.attributes);
        let mut label = l.label.clone().unwrap_or_default();
        self.apply_link_display(l, &mut attrs, &mut label);

        // ── Emit ──
        let mut ci_attrs: Vec<Attr> = vec![("Id", meta.ci_id.clone()), ("Label", label)];
        ci_attrs.extend(meta_attrs(
            l.description.as_deref(),
            l.date.as_deref(),
            l.time.as_deref(),
            l.datetime_description.as_deref(),
            l.ordered,
            l.source_ref.as_deref(),
            l.source_type.as_deref(),
        ));
        push_grades(&mut ci_attrs, grade_one, grade_two, grade_three);
        w.open("ChartItem", &ci_attrs);
        let mut link_attrs: Vec<Attr> =
            vec![("End1Id", end1.to_string()), ("End2Id", end2.to_string())];
        if meta.offset != 0 {
            link_attrs.push(("Offset", meta.offset.to_string()));
        }
        if let Some(cr) = &meta.conn_ref {
            link_attrs.push(("ConnectionReference", cr.clone()));
        }
        if let Some(sg) = &meta.semantic_guid {
            link_attrs.push(("SemanticTypeGuid", sg.clone()));
        }
        w.open("Link", &link_attrs);
        emit_cards(w, &cards);
        let mut style: Vec<Attr> = vec![("Strength", strength)];
        if let Some(a) = arrow {
            style.push(("ArrowStyle", a.to_string()));
        }
        if let Some(lw) = line_width.filter(|w| *w != 1) {
            style.push(("LineWidth", lw.to_string()));
        }
        if let Some(c) = line_color.filter(|c| *c != 0) {
            style.push(("LineColour", c.to_string()));
        }
        if let Some(t) = ltype {
            style.push(("Type", t.to_string()));
            if let Some(r) = &ltype_ref {
                style.push(("LinkTypeReference", r.clone()));
            }
        }
        w.empty("LinkStyle", &style);
        w.close("Link");
        self.emit_attribute_collection(w, &attrs);
        self.emit_ci_style(
            w,
            &l.label_font,
            &l.show,
            l.sub_text_width,
            l.use_sub_text_width,
            l.show_datetime_description,
            None,
            None,
        );
        emit_timezone(w, l.timezone.as_ref());
        w.close("ChartItem");
    }

    /// Apply link display synthesizers (sibling attribute + label) to a link's
    /// resolved attributes/label during lazy emit. The sibling attribute classes
    /// were already registered in `expand_displays`.
    fn apply_link_display(
        &self,
        l: &crate::models::Link,
        attrs: &mut Vec<ResolvedAttr>,
        label: &mut String,
    ) {
        let ltype = l.r#type.as_deref();
        for disp in &self.settings.extra_cfg.display_attribute {
            let (Some(attr_name), Some(template)) =
                (disp.attribute_name.as_deref(), disp.template.as_deref())
            else {
                continue;
            };
            if !crate::transforms::display_kind_matches(disp.kind.as_deref(), true) {
                continue;
            }
            if disp.r#type.as_deref().is_some_and(|t| Some(t) != ltype) {
                continue;
            }
            let Some(ref_id) = self.attribute_classes.get(attr_name).map(|e| e.id.clone()) else {
                continue;
            };
            let metas = crate::transforms::source_metas(&disp.sources);
            let lookup = attr_lookup(attrs);
            if let Some(r) = crate::transforms::render_display(
                &lookup,
                template,
                &metas,
                disp.decimal_separator.as_deref().unwrap_or("."),
                disp.thousand_separator.as_deref().unwrap_or(","),
            ) {
                attrs.push(ResolvedAttr {
                    class: attr_name.to_string(),
                    reference: ref_id,
                    value: Some(r),
                });
            }
        }
        for disp in &self.settings.extra_cfg.display_label {
            let Some(template) = disp.template.as_deref() else {
                continue;
            };
            if !crate::transforms::display_kind_matches(disp.kind.as_deref(), true) {
                continue;
            }
            if disp.r#type.as_deref().is_some_and(|t| Some(t) != ltype) {
                continue;
            }
            if !label.is_empty() && !disp.override_existing.unwrap_or(false) {
                continue;
            }
            let metas = crate::transforms::source_metas(&disp.sources);
            let lookup = attr_lookup(attrs);
            if let Some(r) = crate::transforms::render_display(
                &lookup,
                template,
                &metas,
                disp.decimal_separator.as_deref().unwrap_or("."),
                disp.thousand_separator.as_deref().unwrap_or(","),
            ) {
                *label = r;
            }
        }
    }

    /// Emit one user-defined `<Palette>` and its entry collections, resolving
    /// type names to their registered ids (mirrors Python `_emit_palette`).
    fn emit_user_palette(&self, w: &mut Writer, pal: &crate::models::Palette) {
        let mut attrs: Vec<Attr> = vec![("Name", pal.name.clone())];
        if pal.locked {
            attrs.push(("Locked", "true".into()));
        }
        let empty = pal.attribute_classes.is_empty()
            && pal.attribute_entries.is_empty()
            && pal.entity_types.is_empty()
            && pal.link_types.is_empty();
        if empty {
            w.empty("Palette", &attrs);
            return;
        }
        w.open("Palette", &attrs);
        if !pal.attribute_classes.is_empty() {
            w.open("AttributeClassEntryCollection", &[]);
            for name in &pal.attribute_classes {
                if let Some(e) = self.attribute_classes.get(name) {
                    w.empty(
                        "AttributeClassEntry",
                        &[
                            ("AttributeClass", name.clone()),
                            ("AttributeClassReference", e.id.clone()),
                        ],
                    );
                }
            }
            w.close("AttributeClassEntryCollection");
        }
        if !pal.attribute_entries.is_empty() {
            w.open("AttributeEntryCollection", &[]);
            for ae in &pal.attribute_entries {
                if let Some(e) = self.attribute_classes.get(&ae.name) {
                    let mut a: Vec<Attr> = vec![
                        ("AttributeClass", ae.name.clone()),
                        ("AttributeClassReference", e.id.clone()),
                    ];
                    if let Some(v) = &ae.value {
                        a.push(("Value", v.clone()));
                    }
                    w.empty("AttributeClassEntry", &a);
                }
            }
            w.close("AttributeEntryCollection");
        }
        if !pal.entity_types.is_empty() {
            w.open("EntityTypeEntryCollection", &[]);
            for name in &pal.entity_types {
                if let Some(e) = self.entity_types.get(name) {
                    w.empty(
                        "EntityTypeEntry",
                        &[
                            ("Entity", name.clone()),
                            ("EntityTypeReference", e.id.clone()),
                        ],
                    );
                }
            }
            w.close("EntityTypeEntryCollection");
        }
        if !pal.link_types.is_empty() {
            w.open("LinkTypeEntryCollection", &[]);
            for name in &pal.link_types {
                if let Some(e) = self.link_types.get(name) {
                    w.empty(
                        "LinkTypeEntry",
                        &[
                            ("LinkType", name.clone()),
                            ("LinkTypeReference", e.id.clone()),
                        ],
                    );
                }
            }
            w.close("LinkTypeEntryCollection");
        }
        w.close("Palette");
    }

    /// `<ConnectionCollection>` of the deduped connection styles, in mint order.
    fn emit_connections(&self, w: &mut Writer) {
        if self.connections.is_empty() {
            return;
        }
        w.open("ConnectionCollection", &[]);
        for ((mult, fan_out, tw), id) in &self.connections {
            w.open("Connection", &[("Id", id.clone())]);
            let mut cs: Vec<Attr> = Vec::new();
            if let Some(m) = mult {
                cs.push(("Multiplicity", multiplicity_token(m)));
            }
            if let Some(f) = fan_out {
                cs.push(("FanOut", f.to_string()));
            }
            if let Some(t) = tw {
                cs.push(("ThemeWiring", theme_wiring_token(t)));
            }
            w.empty("ConnectionStyle", &cs);
            w.close("Connection");
        }
        w.close("ConnectionCollection");
    }

    fn emit_palette(&self, w: &mut Writer) {
        // The palette wrapper is always emitted (Python emits an empty
        // `<Palette Name="anxwritter"/>` even with no entries).
        w.open("PaletteCollection", &[]);
        // User-defined palettes replace the auto palette when present.
        if !self.palettes.is_empty() {
            for pal in &self.palettes {
                self.emit_user_palette(w, pal);
            }
            w.close("PaletteCollection");
            return;
        }
        // Attribute classes that the user can add.
        let user_acs: Vec<(&String, &AcEntry)> = self
            .attribute_classes
            .iter()
            .filter(|(_, e)| e.cfg.is_user.unwrap_or(true) && e.cfg.user_can_add.unwrap_or(true))
            .collect();
        // An empty palette is a self-closing `<Palette/>` (matches Python).
        if user_acs.is_empty() && self.entity_types.is_empty() && self.link_types.is_empty() {
            w.empty("Palette", &[("Name", "anxwritter".into())]);
            w.close("PaletteCollection");
            return;
        }
        w.open("Palette", &[("Name", "anxwritter".into())]);
        if !user_acs.is_empty() {
            w.open("AttributeClassEntryCollection", &[]);
            for (name, e) in user_acs {
                w.empty(
                    "AttributeClassEntry",
                    &[
                        ("AttributeClass", name.clone()),
                        ("AttributeClassReference", e.id.clone()),
                    ],
                );
            }
            w.close("AttributeClassEntryCollection");
        }
        if !self.entity_types.is_empty() {
            w.open("EntityTypeEntryCollection", &[]);
            for (name, e) in &self.entity_types {
                w.empty(
                    "EntityTypeEntry",
                    &[
                        ("Entity", name.clone()),
                        ("EntityTypeReference", e.id.clone()),
                    ],
                );
            }
            w.close("EntityTypeEntryCollection");
        }
        if !self.link_types.is_empty() {
            w.open("LinkTypeEntryCollection", &[]);
            for (name, e) in &self.link_types {
                w.empty(
                    "LinkTypeEntry",
                    &[
                        ("LinkType", name.clone()),
                        ("LinkTypeReference", e.id.clone()),
                    ],
                );
            }
            w.close("LinkTypeEntryCollection");
        }
        w.close("Palette");
        w.close("PaletteCollection");
    }
}

// ── Free helpers ────────────────────────────────────────────────────────────

/// Serialize a serde enum to its string value (e.g. `DotStyle::Solid` -> "solid").
fn serde_to_str<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|x| x.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Emit a `<Font>` with the full ANB default set, overlaying any set fields.
fn emit_font_full(w: &mut Writer, f: &crate::models::Font, default_point: i64) {
    let bg = resolve_color(&f.bg_color).unwrap_or(16777215);
    let fg = resolve_color(&f.color).unwrap_or(0);
    w.empty(
        "Font",
        &[
            ("BackColour", bg.to_string()),
            ("Bold", b(f.bold.unwrap_or(false))),
            ("CharSet", "CharSetDefault".into()),
            (
                "FaceName",
                f.name.clone().unwrap_or_else(|| "Tahoma".into()),
            ),
            ("FontColour", fg.to_string()),
            ("Italic", b(f.italic.unwrap_or(false))),
            ("PointSize", f.size.unwrap_or(default_point).to_string()),
            ("Strikeout", b(f.strikeout.unwrap_or(false))),
            ("Underline", b(f.underline.unwrap_or(false))),
        ],
    );
}

/// Shared `<ChartItem>` metadata attributes (description, date/time, source),
/// emitted in XSD order after Id/Label and before grades.
#[allow(clippy::too_many_arguments)]
fn meta_attrs(
    description: Option<&str>,
    date: Option<&str>,
    time: Option<&str>,
    datetime_description: Option<&str>,
    ordered: Option<bool>,
    source_ref: Option<&str>,
    source_type: Option<&str>,
) -> Vec<Attr<'static>> {
    let mut a: Vec<Attr<'static>> = Vec::new();
    if let Some(d) = description.filter(|s| !s.is_empty()) {
        a.push(("Description", d.to_string()));
    }
    if let Some((dt, ds, ts)) = crate::datetime::build_datetime(date, time) {
        // DateSet / TimeSet are emitted only when true (matching upstream).
        if ds {
            a.push(("DateSet", "true".to_string()));
        }
        if ts {
            a.push(("TimeSet", "true".to_string()));
        }
        a.push(("DateTime", dt));
    }
    if let Some(x) = datetime_description.filter(|s| !s.is_empty()) {
        a.push(("DateTimeDescription", x.to_string()));
    }
    if ordered == Some(true) {
        a.push(("Ordered", "true".to_string()));
    }
    if let Some(s) = source_ref.filter(|s| !s.is_empty()) {
        a.push(("SourceReference", s.to_string()));
    }
    if let Some(s) = source_type.filter(|s| !s.is_empty()) {
        a.push(("SourceType", s.to_string()));
    }
    a
}

/// Emit a `<CardCollection>` of `<Card>` elements (with optional `<TimeZone>`).
fn emit_cards(w: &mut Writer, cards: &[crate::resolved::ResolvedCard]) {
    if cards.is_empty() {
        return;
    }
    w.open("CardCollection", &[]);
    for c in cards {
        let mut a: Vec<Attr> = Vec::new();
        if let Some(s) = &c.summary {
            a.push(("Summary", s.clone()));
        }
        if c.date_set {
            a.push(("DateSet", "true".to_string()));
        }
        if c.time_set {
            a.push(("TimeSet", "true".to_string()));
        }
        if let Some(dt) = &c.datetime {
            a.push(("DateTime", dt.clone()));
        }
        if let Some(g) = c.grade_one {
            a.push(("GradeOneIndex", g.to_string()));
        }
        if let Some(g) = c.grade_two {
            a.push(("GradeTwoIndex", g.to_string()));
        }
        if let Some(g) = c.grade_three {
            a.push(("GradeThreeIndex", g.to_string()));
        }
        if let Some(s) = &c.source_ref {
            a.push(("SourceReference", s.clone()));
        }
        if let Some(s) = &c.source_type {
            a.push(("SourceType", s.clone()));
        }
        if let Some(s) = &c.description {
            a.push(("Text", s.clone()));
        }
        if let Some(s) = &c.datetime_description {
            a.push(("DateTimeDescription", s.clone()));
        }
        if c.timezone_id.is_some() || c.timezone_name.is_some() {
            w.open("Card", &a);
            let mut tz: Vec<Attr> = Vec::new();
            if let Some(id) = c.timezone_id {
                tz.push(("UniqueID", id.to_string()));
            }
            if let Some(n) = &c.timezone_name {
                tz.push(("Name", n.clone()));
            }
            w.empty("TimeZone", &tz);
            w.close("Card");
        } else {
            w.empty("Card", &a);
        }
    }
    w.close("CardCollection");
}

/// Build a `class_name -> value` map from resolved attributes (for display
/// template rendering).
fn attr_lookup(attrs: &[ResolvedAttr]) -> IndexMap<String, String> {
    attrs
        .iter()
        .map(|a| (a.class.clone(), a.value.clone().unwrap_or_default()))
        .collect()
}

fn push_grades(attrs: &mut Vec<Attr>, g1: Option<i64>, g2: Option<i64>, g3: Option<i64>) {
    if let Some(g) = g1 {
        attrs.push(("GradeOneIndex", g.to_string()));
    }
    if let Some(g) = g2 {
        attrs.push(("GradeTwoIndex", g.to_string()));
    }
    if let Some(g) = g3 {
        attrs.push(("GradeThreeIndex", g.to_string()));
    }
}

/// Gather a borrowed, representation-unified view of every entity in the data
/// groups, in emission order — no cloning (see [`EntityRef`]).
fn collect_entity_refs(data: &ChartData) -> Vec<EntityRef<'_>> {
    let g = &data.entities;
    let mut out = Vec::new();
    out.extend(g.icons.iter().map(EntityRef::Icon));
    out.extend(g.boxes.iter().map(EntityRef::Box));
    out.extend(g.circles.iter().map(EntityRef::Circle));
    out.extend(g.theme_lines.iter().map(EntityRef::ThemeLine));
    out.extend(g.event_frames.iter().map(EntityRef::EventFrame));
    out.extend(g.text_blocks.iter().map(EntityRef::TextBlock));
    out.extend(g.labels.iter().map(EntityRef::Label));
    out
}

fn box_dims(width: Option<i64>, height: Option<i64>, depth: Option<i64>) -> Vec<Attr<'static>> {
    let mut a = Vec::new();
    if let Some(w) = width {
        a.push(("Width", w.to_string()));
    }
    if let Some(h) = height {
        a.push(("Height", h.to_string()));
    }
    if let Some(d) = depth {
        a.push(("Depth", d.to_string()));
    }
    a
}

fn push_icon_override(style: &mut Vec<Attr>, icon: &Option<String>) {
    if let Some(name) = icon {
        style.push(("OverrideTypeIcon", "true".to_string()));
        style.push(("TypeIconName", name.clone()));
    }
}

fn push_type_ref(style: &mut Vec<Attr>, type_ref: &Option<String>) {
    if let Some(r) = type_ref {
        style.push(("EntityTypeReference", r.clone()));
    }
}

fn push_fill(
    style: &mut Vec<Attr>,
    bg: Option<u32>,
    filled: Option<bool>,
    line_width: Option<i64>,
) {
    if let Some(c) = bg {
        style.push(("BackColour", c.to_string()));
    }
    if let Some(v) = filled {
        style.push(("Filled", b(v)));
    }
    if let Some(w) = line_width {
        style.push(("LineWidth", w.to_string()));
    }
}

fn push_wh(style: &mut Vec<Attr>, width: Option<i64>, height: Option<i64>) {
    if let Some(w) = width {
        style.push(("Width", (w as f64 / 100.0).to_string()));
    }
    if let Some(h) = height {
        style.push(("Height", (h as f64 / 100.0).to_string()));
    }
}
