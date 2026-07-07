//! anxwritter — write i2 Analyst's Notebook Exchange (`.anx`) files from typed
//! data, JSON, or YAML.
//!
//! A faithful Rust port of the [`anxwritter`] Python library. `.anx` files are
//! UTF-16 XML documents that i2 Analyst's Notebook 9+ opens directly; this crate
//! builds them from in-memory chart objects, JSON, or YAML, with output
//! content-identical to the Python reference.
//!
//! This crate tracks a specific upstream release — see
//! [`TARGET_ANXWRITTER_VERSION`]. For chart concepts, the `.anx` format, and the
//! full set of config/style options, refer to the Python library's documentation;
//! this crate intentionally keeps its own docs thin and defers to upstream.
//!
//! # Example
//!
//! ```no_run
//! use anxwritter::api::write_anx;
//! use anxwritter::input::{ChartData, Config};
//!
//! let config = Config::from_json("{}")?;
//! let data = ChartData::from_json(
//!     r#"{"entities":{"icons":[{"id":"alice","type":"Person"}]},"links":[]}"#,
//! )?;
//! let file = std::fs::File::create("chart.anx")?;
//! // Validate, then stream UTF-16 `.anx` bytes straight to the writer.
//! write_anx(&config, &data, file, true)?;
//! # Ok::<(), anxwritter::Error>(())
//! ```
//!
//! [`anxwritter`]: https://github.com/gustavo-gkmi/anxwritter

/// The upstream `anxwritter` (Python) release this crate targets. The emitted
/// `.anx` provenance comment and output format track this version.
pub const TARGET_ANXWRITTER_VERSION: &str = "1.24.2";

pub mod api;
pub mod builder;
pub mod color;
pub mod config_layering;
pub mod custom_icons;
pub mod datetime;
pub mod discovery;
pub mod entities;
pub mod enums;
pub mod error;
pub mod ids;
pub mod input;
pub mod interop;
pub mod layout;
pub mod models;
pub mod resolved;
pub mod semantic;
pub mod sugar;
pub mod transforms;
pub mod validation;
pub mod value;
pub mod xml;

pub use api::{build_anx, render_xml, write_anx};
pub use builder::Builder;
pub use sugar::EntityExt;

pub use color::{ColorValue, NAMED_COLORS};
pub use config_layering::{CascadeMode, ConfigStack};
pub use entities::{
    Box, Circle, Entity, EntityCommon, EventFrame, Icon, Label, TextBlock, ThemeLine,
};
pub use enums::{
    ArrowStyle, AttributeType, Color, ColorSpace, DotStyle, Enlargement, EnumMeta, IntensityScale,
    LegendItemType, MergeBehaviour, MissingPolicy, Multiplicity, Representation, ThemeWiring,
};
pub use error::{AnxValidationError, Error, ErrorType, Result, ValidationError};
pub use input::{ChartData, Config, EntityGroups};
pub use models::{
    AttributeClass, Card, CustomProperty, DateTimeFormat, EntityType, Font, Frame, GradeCollection,
    GradeRef, LegendItem, Link, LinkType, Palette, Settings, Show, Strength, StrengthCollection,
    TimeZone, Validator,
};
pub use value::{AttrValue, InferredType};

/// Common imports for building charts ergonomically: `use anxwritter::prelude::*;`.
///
/// Brings in the chart types, fluent entity/link/card constructors (via
/// [`EntityExt`] and inherent `new`/setter methods), and the build entry points.
/// Note: the [`entities::Box`] entity type is intentionally *not* glob-imported
/// here (it would shadow [`std::boxed::Box`]); import it explicitly if needed.
pub mod prelude {
    pub use crate::api::{build_anx, write_anx};
    pub use crate::builder::Builder;
    pub use crate::color::ColorValue;
    pub use crate::entities::{Circle, Entity, EventFrame, Icon, Label, TextBlock, ThemeLine};
    pub use crate::input::{ChartData, Config};
    pub use crate::models::{Card, GradeRef, Link};
    pub use crate::sugar::EntityExt;
    pub use crate::value::AttrValue;
}
