//! Ergonomic constructors and fluent setters over the data model — pure additive
//! sugar, no new state. Build charts in Rust without hand-assembling structs:
//!
//! ```
//! use anxwritter::prelude::*;
//!
//! let mut data = ChartData::default();
//! data.add_icon(Icon::new("alice", "Person").label("Alice").attr("Phone", "555-0100"))
//!     .add_icon(Icon::new("bob", "Person"))
//!     .add_link(Link::new("alice", "bob").link_type("Call").directed());
//!
//! let xml = anxwritter::Builder::new(&Config::default()).build(&data);
//! assert!(xml.contains("Identity=\"alice\""));
//! ```

use crate::color::ColorValue;
use crate::entities::{
    Box, Circle, Entity, EntityCommon, EventFrame, Icon, Label, TextBlock, ThemeLine,
};
use crate::input::ChartData;
use crate::models::{Card, GradeRef, Link};
use crate::value::AttrValue;

// ── Value conversions (make `.attr`, `.color`, grades terse) ────────────────

impl From<&str> for AttrValue {
    fn from(s: &str) -> Self {
        AttrValue::Str(s.to_string())
    }
}
impl From<String> for AttrValue {
    fn from(s: String) -> Self {
        AttrValue::Str(s)
    }
}
impl From<i64> for AttrValue {
    fn from(n: i64) -> Self {
        AttrValue::Int(n)
    }
}
impl From<i32> for AttrValue {
    fn from(n: i32) -> Self {
        AttrValue::Int(n as i64)
    }
}
impl From<f64> for AttrValue {
    fn from(n: f64) -> Self {
        AttrValue::Float(n)
    }
}
impl From<bool> for AttrValue {
    fn from(b: bool) -> Self {
        AttrValue::Bool(b)
    }
}

impl From<u32> for ColorValue {
    fn from(v: u32) -> Self {
        ColorValue::Int(v)
    }
}
impl From<&str> for ColorValue {
    /// A named colour (`"Red"`) or `#RRGGBB` hex — resolved at build time.
    fn from(s: &str) -> Self {
        ColorValue::Str(s.to_string())
    }
}
impl From<String> for ColorValue {
    fn from(s: String) -> Self {
        ColorValue::Str(s)
    }
}

impl From<i64> for GradeRef {
    fn from(i: i64) -> Self {
        GradeRef::Index(i)
    }
}
impl From<&str> for GradeRef {
    fn from(s: &str) -> Self {
        GradeRef::Name(s.to_string())
    }
}
impl From<String> for GradeRef {
    fn from(s: String) -> Self {
        GradeRef::Name(s)
    }
}

// ── Entity construction + shared fluent setters ─────────────────────────────

/// Fluent setters shared by every entity representation (operate on the common
/// fields). Import via `anxwritter::prelude::*` or `use anxwritter::EntityExt`.
pub trait EntityExt: Sized {
    /// Borrow the shared fields mutably (the one method each impl provides).
    fn common_mut(&mut self) -> &mut EntityCommon;

    fn label(mut self, v: impl Into<String>) -> Self {
        self.common_mut().label = Some(v.into());
        self
    }
    fn attr(mut self, name: impl Into<String>, value: impl Into<AttrValue>) -> Self {
        self.common_mut()
            .attributes
            .insert(name.into(), value.into());
        self
    }
    fn position(mut self, x: i64, y: i64) -> Self {
        let c = self.common_mut();
        c.x = Some(x);
        c.y = Some(y);
        self
    }
    fn date(mut self, v: impl Into<String>) -> Self {
        self.common_mut().date = Some(v.into());
        self
    }
    fn time(mut self, v: impl Into<String>) -> Self {
        self.common_mut().time = Some(v.into());
        self
    }
    fn description(mut self, v: impl Into<String>) -> Self {
        self.common_mut().description = Some(v.into());
        self
    }
    fn strength(mut self, v: impl Into<String>) -> Self {
        self.common_mut().strength = Some(v.into());
        self
    }
    fn grade_one(mut self, v: impl Into<GradeRef>) -> Self {
        self.common_mut().grade_one = Some(v.into());
        self
    }
    fn grade_two(mut self, v: impl Into<GradeRef>) -> Self {
        self.common_mut().grade_two = Some(v.into());
        self
    }
    fn grade_three(mut self, v: impl Into<GradeRef>) -> Self {
        self.common_mut().grade_three = Some(v.into());
        self
    }
    fn card(mut self, c: Card) -> Self {
        self.common_mut().cards.push(c);
        self
    }
}

macro_rules! entity_sugar {
    ($($t:ident),* $(,)?) => {$(
        impl $t {
            /// Create the entity with its required `id` and `type`; all else default.
            pub fn new(id: impl Into<String>, ty: impl Into<String>) -> Self {
                let mut e = Self::default();
                e.common.id = id.into();
                e.common.r#type = ty.into();
                e
            }
        }
        impl EntityExt for $t {
            fn common_mut(&mut self) -> &mut EntityCommon { &mut self.common }
        }
    )*};
}
entity_sugar!(Icon, Box, Circle, ThemeLine, EventFrame, TextBlock, Label);

impl Icon {
    /// Icon shading colour (named, `#RRGGBB`, or COLORREF int).
    pub fn color(mut self, c: impl Into<ColorValue>) -> Self {
        self.color = Some(c.into());
        self
    }
    /// Per-entity icon-name override (`OverrideTypeIcon`/`TypeIconName`).
    pub fn icon_name(mut self, name: impl Into<String>) -> Self {
        self.icon = Some(name.into());
        self
    }
}

// ── Link construction + fluent setters ──────────────────────────────────────

impl Link {
    /// Create a link between two entity ids; type/style default.
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Link {
            from_id: from.into(),
            to_id: to.into(),
            ..Default::default()
        }
    }
    pub fn link_type(mut self, t: impl Into<String>) -> Self {
        self.r#type = Some(t.into());
        self
    }
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }
    pub fn arrow(mut self, a: impl Into<String>) -> Self {
        self.arrow = Some(a.into());
        self
    }
    /// Shorthand for an arrow on the head (`->`).
    pub fn directed(self) -> Self {
        self.arrow("->")
    }
    pub fn attr(mut self, name: impl Into<String>, value: impl Into<AttrValue>) -> Self {
        self.attributes.insert(name.into(), value.into());
        self
    }
    pub fn strength(mut self, s: impl Into<String>) -> Self {
        self.strength = Some(s.into());
        self
    }
    pub fn line_color(mut self, c: impl Into<ColorValue>) -> Self {
        self.line_color = Some(c.into());
        self
    }
    pub fn line_width(mut self, w: i64) -> Self {
        self.line_width = Some(w);
        self
    }
    pub fn date(mut self, d: impl Into<String>) -> Self {
        self.date = Some(d.into());
        self
    }
    pub fn time(mut self, t: impl Into<String>) -> Self {
        self.time = Some(t.into());
        self
    }
    pub fn grade_one(mut self, g: impl Into<GradeRef>) -> Self {
        self.grade_one = Some(g.into());
        self
    }
    pub fn grade_two(mut self, g: impl Into<GradeRef>) -> Self {
        self.grade_two = Some(g.into());
        self
    }
    pub fn grade_three(mut self, g: impl Into<GradeRef>) -> Self {
        self.grade_three = Some(g.into());
        self
    }
    pub fn multiplicity(mut self, m: impl Into<String>) -> Self {
        self.multiplicity = Some(m.into());
        self
    }
    pub fn theme_wiring(mut self, w: impl Into<String>) -> Self {
        self.theme_wiring = Some(w.into());
        self
    }
    pub fn card(mut self, c: Card) -> Self {
        self.cards.push(c);
        self
    }
}

// ── Card construction + fluent setters ──────────────────────────────────────

impl Card {
    pub fn new(summary: impl Into<String>) -> Self {
        Card {
            summary: Some(summary.into()),
            ..Default::default()
        }
    }
    pub fn date(mut self, d: impl Into<String>) -> Self {
        self.date = Some(d.into());
        self
    }
    pub fn time(mut self, t: impl Into<String>) -> Self {
        self.time = Some(t.into());
        self
    }
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = Some(d.into());
        self
    }
    /// Set the evidence source type (and optionally a source reference).
    pub fn source(mut self, source_type: impl Into<String>) -> Self {
        self.source_type = Some(source_type.into());
        self
    }
    pub fn source_ref(mut self, r: impl Into<String>) -> Self {
        self.source_ref = Some(r.into());
        self
    }
}

// ── ChartData accumulation helpers (chainable on a &mut binding) ─────────────

impl ChartData {
    pub fn add_icon(&mut self, e: Icon) -> &mut Self {
        self.entities.icons.push(e);
        self
    }
    pub fn add_box(&mut self, e: Box) -> &mut Self {
        self.entities.boxes.push(e);
        self
    }
    pub fn add_circle(&mut self, e: Circle) -> &mut Self {
        self.entities.circles.push(e);
        self
    }
    pub fn add_theme_line(&mut self, e: ThemeLine) -> &mut Self {
        self.entities.theme_lines.push(e);
        self
    }
    pub fn add_event_frame(&mut self, e: EventFrame) -> &mut Self {
        self.entities.event_frames.push(e);
        self
    }
    pub fn add_text_block(&mut self, e: TextBlock) -> &mut Self {
        self.entities.text_blocks.push(e);
        self
    }
    pub fn add_label(&mut self, e: Label) -> &mut Self {
        self.entities.labels.push(e);
        self
    }
    pub fn add_link(&mut self, l: Link) -> &mut Self {
        self.links.push(l);
        self
    }
    /// Add a loose card (routed to an entity/link by its `entity_id`/`link_id`).
    pub fn add_loose_card(&mut self, c: Card) -> &mut Self {
        self.loose_cards.push(c);
        self
    }
    /// Add any entity, routed to its representation group by variant.
    pub fn add(&mut self, e: Entity) -> &mut Self {
        match e {
            Entity::Icon(x) => self.entities.icons.push(x),
            Entity::Box(x) => self.entities.boxes.push(x),
            Entity::Circle(x) => self.entities.circles.push(x),
            Entity::ThemeLine(x) => self.entities.theme_lines.push(x),
            Entity::EventFrame(x) => self.entities.event_frames.push(x),
            Entity::TextBlock(x) => self.entities.text_blocks.push(x),
            Entity::Label(x) => self.entities.labels.push(x),
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    #[test]
    fn fluent_build_produces_valid_chart() {
        let mut data = ChartData::default();
        data.add_icon(
            Icon::new("alice", "Person")
                .label("Alice")
                .attr("Phone", "555-0100")
                .attr("Age", 30)
                .attr("Active", true)
                .color("Red")
                .position(-80, 0)
                .card(Card::new("Sighting").date("2026-01-05").source("Witness")),
        )
        .add_icon(Icon::new("bob", "Person"))
        .add_link(
            Link::new("alice", "bob")
                .link_type("Call")
                .directed()
                .attr("weight", 5),
        );

        let xml = crate::Builder::new(&Config::default()).build(&data);
        assert!(xml.contains("Identity=\"alice\""));
        assert!(xml.contains("Value=\"555-0100\""));
        assert!(xml.contains("ArrowStyle=\"ArrowOnHead\""));
        assert!(xml.contains("<Card "));
    }
}
