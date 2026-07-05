//! Top-level input schemas for JSON/YAML.
//!
//! A `.anx` build is driven by two kinds of document, matching the CLI's
//! layering: zero or more **config** layers (type definitions, settings,
//! strengths, grades, legend, semantic types) and one **data** layer (the
//! entities and links). Both are plain JSON/YAML; unknown keys are ignored so
//! that forward-compatible extras (e.g. `cascade`) don't break loading.

use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

use crate::entities::{Box, Circle, EventFrame, Icon, Label, TextBlock, ThemeLine};
use crate::models::{
    AttributeClass, Card, DateTimeFormat, EntityType, GradeCollection, LegendItem, Link, LinkType,
    Palette, SemanticEntity, SemanticLink, SemanticProperty, Settings, StrengthCollection,
    Validator,
};

/// Entity rows grouped by representation, keyed exactly as the data schema
/// (`entities.icons`, `entities.boxes`, ...).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EntityGroups {
    pub icons: Vec<Icon>,
    pub boxes: Vec<Box>,
    pub circles: Vec<Circle>,
    pub theme_lines: Vec<ThemeLine>,
    pub event_frames: Vec<EventFrame>,
    pub text_blocks: Vec<TextBlock>,
    pub labels: Vec<Label>,
}

/// A data layer: the chart's entities, links, and any loose cards.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChartData {
    pub entities: EntityGroups,
    pub links: Vec<Link>,
    pub loose_cards: Vec<Card>,
}

/// A config layer: type definitions, settings, and chart-wide collections.
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub settings: Option<Settings>,
    pub entity_types: Vec<EntityType>,
    pub link_types: Vec<LinkType>,
    pub attribute_classes: Vec<AttributeClass>,
    pub datetime_formats: Vec<DateTimeFormat>,
    pub strengths: Option<StrengthCollection>,
    pub grades_one: Option<GradeCollection>,
    pub grades_two: Option<GradeCollection>,
    pub grades_three: Option<GradeCollection>,
    pub source_types: Vec<String>,
    pub legend_items: Vec<LegendItem>,
    pub palettes: Vec<Palette>,
    pub semantic_entities: Vec<SemanticEntity>,
    pub semantic_links: Vec<SemanticLink>,
    pub semantic_properties: Vec<SemanticProperty>,
    pub validators: Vec<Validator>,
    pub custom_entity_icons: Vec<crate::models::CustomIconEntry>,
    pub custom_attribute_icons: Vec<crate::models::CustomIconEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // The bundled upstream examples are the ground-truth input schema.
    const DATA_JSON: &str = include_str!("../tests/fixtures/example_data.json");
    const CONFIG_JSON: &str = include_str!("../tests/fixtures/example_config.json");

    #[test]
    fn deserializes_example_data() {
        let data: ChartData = serde_json::from_str(DATA_JSON).unwrap();
        assert_eq!(data.entities.icons.len(), 8);
        assert_eq!(data.links.len(), 8);
        // Spot-check a known row and a boolean attribute (Flag inference).
        let alex = &data.entities.icons[0];
        assert_eq!(alex.common.id, "alex_carter");
        assert_eq!(alex.common.r#type, "Person");
        assert!(alex.common.attributes.contains_key("Active"));
    }

    #[test]
    fn deserializes_example_yaml() {
        let data: ChartData =
            serde_yaml_ng::from_str(include_str!("../tests/fixtures/example_data.yaml")).unwrap();
        assert!(!data.entities.icons.is_empty());
        let cfg: Config =
            serde_yaml_ng::from_str(include_str!("../tests/fixtures/example_config.yaml")).unwrap();
        assert!(!cfg.entity_types.is_empty());
    }

    #[test]
    fn deserializes_example_config() {
        let cfg: Config = serde_json::from_str(CONFIG_JSON).unwrap();
        assert_eq!(cfg.entity_types.len(), 5);
        assert_eq!(cfg.link_types.len(), 5);
        assert_eq!(cfg.attribute_classes.len(), 6);
        assert_eq!(cfg.source_types.len(), 5);
        // legend_items use the `label` key aliased to `name`.
        assert_eq!(cfg.legend_items.len(), 3);
        assert_eq!(cfg.legend_items[0].name, "Person");
        // strengths is an object with a default + items.
        let s = cfg.strengths.as_ref().unwrap();
        assert_eq!(s.default.as_deref(), Some("Confirmed"));
        assert_eq!(s.items.len(), 3);
    }
}
