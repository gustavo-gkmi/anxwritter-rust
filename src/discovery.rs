//! Programmatic discovery tables for building a service `/meta` endpoint.
//!
//! Everything a downstream crate needs to enumerate the crate's accepted
//! vocabulary — enum value lists, the named-color map, and the arrange
//! algorithm/alias tables — **generated from the crate itself**, so a service's
//! discovery payload can never drift from what the emitter actually accepts.
//!
//! This mirrors what the Python service's `app/meta.py` builds by introspecting
//! the `anxwritter` library.
//!
//! # Example
//!
//! ```
//! use anxwritter::discovery;
//!
//! // Every public enum with its serialized string values.
//! for e in discovery::enums() {
//!     println!("{}: {:?}", e.name, e.values);
//! }
//!
//! // name -> COLORREF map, and the arrange algorithm list + alias table.
//! let colors = discovery::named_colors();
//! let algos = discovery::arrange_algorithms();
//! let aliases = discovery::arrange_aliases();
//! # let _ = (colors, algos, aliases);
//! ```

use crate::color::NAMED_COLORS;
use crate::enums::{
    ArrowStyle, AttributeType, Color, ColorSpace, DotStyle, Enlargement, IntensityScale,
    LegendItemType, MergeBehaviour, MissingPolicy, Multiplicity, Representation, ThemeWiring,
};

pub use crate::enums::EnumMeta;
pub use crate::layout::{ARRANGE_ALGORITHMS, ARRANGE_ALIASES};

/// One discoverable enum: its type name and its serialized string values.
///
/// `name` is the Python enum class name (`"ArrowStyle"`, `"DotStyle"`, …) so a
/// service can key its payload the same way the Python `meta.py` does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumInfo {
    /// The enum's type name (matches the Python enum class, e.g. `"ArrowStyle"`).
    pub name: &'static str,
    /// The serialized string value of every variant, in declaration order.
    pub values: Vec<&'static str>,
}

/// Every public config/style enum with its serialized values, in the same set
/// the Python service enumerates for `/v1/meta`.
///
/// Values come straight from [`EnumMeta`], which a test pins to the serde
/// serialization — so this list stays in lock-step with the wire format.
pub fn enums() -> Vec<EnumInfo> {
    fn info<T: EnumMeta>(name: &'static str) -> EnumInfo {
        EnumInfo {
            name,
            values: T::values(),
        }
    }
    vec![
        info::<ArrowStyle>("ArrowStyle"),
        info::<DotStyle>("DotStyle"),
        info::<Multiplicity>("Multiplicity"),
        info::<Representation>("Representation"),
        info::<ThemeWiring>("ThemeWiring"),
        info::<Enlargement>("Enlargement"),
        info::<IntensityScale>("IntensityScale"),
        info::<MergeBehaviour>("MergeBehaviour"),
        info::<LegendItemType>("LegendItemType"),
        info::<MissingPolicy>("MissingPolicy"),
        info::<AttributeType>("AttributeType"),
        info::<ColorSpace>("ColorSpace"),
        info::<Color>("Color"),
    ]
}

/// The named-color table: `(normalized_name, COLORREF)`. Names are
/// pre-normalized (lowercase, `-`/space -> `_`), matching the crate's `Color`
/// enum values. For the original ANB display casing Python's `NAMED_COLORS` dict
/// exposes (`"Light Orange"`), use [`named_colors_display`].
pub fn named_colors() -> &'static [(&'static str, u32)] {
    NAMED_COLORS
}

/// The named-color table keyed by Python's original display casing:
/// `(display_name, COLORREF)` — e.g. `("Light Orange", …)`, `("Blue-Grey", …)`.
/// Parallel to [`named_colors`] (same order/colorrefs). Emit these when a `/meta`
/// endpoint wants key-parity with the Python `NAMED_COLORS` dict.
pub fn named_colors_display() -> &'static [(&'static str, u32)] {
    crate::color::NAMED_COLORS_DISPLAY
}

/// The canonical arrange algorithm keys (`fr`, `forceatlas2`, `tree`, `radial`,
/// `circle`, `grid`, `random`).
pub fn arrange_algorithms() -> &'static [&'static str] {
    ARRANGE_ALGORITHMS
}

/// The arrange **alias table**: `(alias, canonical)` — e.g. `("fa2",
/// "forceatlas2")`, `("reingold_tilford", "tree")`. Mirrors the Python
/// `layouts._ALIASES`.
pub fn arrange_aliases() -> &'static [(&'static str, &'static str)] {
    ARRANGE_ALIASES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enums_cover_the_required_set() {
        let names: Vec<&str> = enums().iter().map(|e| e.name).collect();
        for required in [
            "ArrowStyle",
            "DotStyle",
            "Multiplicity",
            "Representation",
            "ThemeWiring",
            "Enlargement",
            "IntensityScale",
            "MergeBehaviour",
            "LegendItemType",
            "MissingPolicy",
            "AttributeType",
            "ColorSpace",
        ] {
            assert!(names.contains(&required), "missing enum {required}");
        }
        // A couple of concrete values, to catch accidental reordering/renames.
        let by_name = |n: &str| enums().into_iter().find(|e| e.name == n).unwrap().values;
        assert_eq!(by_name("ArrowStyle"), vec!["head", "tail", "both"]);
        assert_eq!(by_name("ColorSpace"), vec!["rgb", "rgb_linear", "hsl"]);
    }

    #[test]
    fn arrange_alias_table_matches_python() {
        // Every alias resolves to a canonical algorithm; the key aliases the
        // server relies on are present.
        for (alias, canonical) in arrange_aliases() {
            assert!(
                arrange_algorithms().contains(canonical),
                "alias {alias} -> {canonical} not a canonical algorithm"
            );
        }
        let has = |a: &str, c: &str| arrange_aliases().contains(&(a, c));
        assert!(has("fa2", "forceatlas2"));
        assert!(has("fruchterman_reingold", "fr"));
        assert!(has("reingold_tilford", "tree"));
        assert!(has("tidy_tree", "tree"));
    }

    #[test]
    fn named_colors_are_present() {
        let map = named_colors();
        assert_eq!(map.len(), 40);
        assert!(map.contains(&("white", 0x00FF_FFFF)));
        assert!(map.contains(&("black", 0)));
    }

    #[test]
    fn named_colors_display_preserves_python_casing() {
        let map = named_colors_display();
        assert_eq!(map.len(), 40);
        assert!(map.contains(&("Light Orange", 0x0000_99FF)));
        assert!(map.contains(&("Blue-Grey", 0x0099_6666)));
        assert!(map.contains(&("White", 0x00FF_FFFF)));
    }
}
