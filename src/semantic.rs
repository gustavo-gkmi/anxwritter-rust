//! Semantic type resolution and the `<lcx:LibraryCatalogue>`, mirroring
//! `anxwritter/semantic.py`.
//!
//! Custom semantic types (entities/links/properties) are assigned deterministic
//! GUIDs (`entity:Name` / `link:Name` / `property:Name` keys). References on
//! entities/types/attribute-classes resolve a name to its GUID; the catalogue
//! emits the minimal set of referenced types plus their ancestors, in
//! ancestors-first (topological) order.

use indexmap::{IndexMap, IndexSet};

use crate::input::Config;
use crate::interop::{
    generate_guid, ROOT_ABSTRACT_DT, ROOT_ABSTRACT_FLAG, ROOT_ABSTRACT_NUM, ROOT_ABSTRACT_TEXT,
    ROOT_ENTITY, ROOT_LINK,
};

/// A resolved custom semantic type ready for catalogue emission.
#[derive(Debug, Clone)]
pub struct CatType {
    pub guid: String,
    pub parent_guid: Option<String>,
    pub abstract_: bool,
    pub name: String,
    pub synonyms: Vec<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
struct Custom {
    guid: String,
    parent_guid: Option<String>,
    abstract_: bool,
    name: String,
    synonyms: Vec<String>,
    description: Option<String>,
}

/// Resolves semantic-type names to GUIDs and builds the catalogue.
#[derive(Debug, Default)]
pub struct SemanticResolver {
    entities: IndexMap<String, Custom>,
    links: IndexMap<String, Custom>,
    properties: IndexMap<String, Custom>,
    guid_to_name: IndexMap<String, String>,
    referenced_type_guids: IndexSet<String>,
    referenced_property_guids: IndexSet<String>,
}

fn root_names() -> Vec<(&'static str, &'static str)> {
    vec![
        ("Entity", ROOT_ENTITY),
        ("Link", ROOT_LINK),
        ("Abstract Text", ROOT_ABSTRACT_TEXT),
        ("Abstract Number", ROOT_ABSTRACT_NUM),
        ("Abstract Date & Time", ROOT_ABSTRACT_DT),
        ("Abstract Flag", ROOT_ABSTRACT_FLAG),
    ]
}

impl SemanticResolver {
    /// Build a resolver from the config's semantic type definitions.
    pub fn new(config: &Config) -> Self {
        let mut r = SemanticResolver::default();
        for (n, g) in root_names() {
            r.guid_to_name.insert(g.to_string(), n.to_string());
        }

        // First pass: assign guids and name->guid maps (per tree).
        let mut ent_name_guid: IndexMap<String, String> =
            IndexMap::from([("Entity".to_string(), ROOT_ENTITY.to_string())]);
        for se in &config.semantic_entities {
            let g = se
                .guid
                .clone()
                .unwrap_or_else(|| generate_guid(&format!("entity:{}", se.name)));
            ent_name_guid.insert(se.name.clone(), g);
        }
        let mut lnk_name_guid: IndexMap<String, String> =
            IndexMap::from([("Link".to_string(), ROOT_LINK.to_string())]);
        for sl in &config.semantic_links {
            let g = sl
                .guid
                .clone()
                .unwrap_or_else(|| generate_guid(&format!("link:{}", sl.name)));
            lnk_name_guid.insert(sl.name.clone(), g);
        }
        let mut prop_name_guid: IndexMap<String, String> = IndexMap::from([
            ("Abstract Text".to_string(), ROOT_ABSTRACT_TEXT.to_string()),
            ("Abstract Number".to_string(), ROOT_ABSTRACT_NUM.to_string()),
            (
                "Abstract Date & Time".to_string(),
                ROOT_ABSTRACT_DT.to_string(),
            ),
            ("Abstract Flag".to_string(), ROOT_ABSTRACT_FLAG.to_string()),
        ]);
        for sp in &config.semantic_properties {
            let g = sp
                .guid
                .clone()
                .unwrap_or_else(|| generate_guid(&format!("property:{}", sp.name)));
            prop_name_guid.insert(sp.name.clone(), g);
        }

        // Second pass: resolve parents and store custom records.
        let parent_of = |kind: &str, map: &IndexMap<String, String>| -> Option<String> {
            if kind.is_empty() {
                None
            } else if kind.starts_with("guid") {
                Some(kind.to_string())
            } else {
                map.get(kind).cloned()
            }
        };
        for se in &config.semantic_entities {
            let guid = ent_name_guid[&se.name].clone();
            let c = Custom {
                guid: guid.clone(),
                parent_guid: parent_of(&se.kind_of, &ent_name_guid),
                abstract_: se.abstract_,
                name: se.name.clone(),
                synonyms: se.synonyms.clone().unwrap_or_default(),
                description: se.description.clone(),
            };
            r.guid_to_name.insert(guid, se.name.clone());
            r.entities.insert(se.name.clone(), c);
        }
        for sl in &config.semantic_links {
            let guid = lnk_name_guid[&sl.name].clone();
            let c = Custom {
                guid: guid.clone(),
                parent_guid: parent_of(&sl.kind_of, &lnk_name_guid),
                abstract_: sl.abstract_,
                name: sl.name.clone(),
                synonyms: sl.synonyms.clone().unwrap_or_default(),
                description: sl.description.clone(),
            };
            r.guid_to_name.insert(guid, sl.name.clone());
            r.links.insert(sl.name.clone(), c);
        }
        for sp in &config.semantic_properties {
            let guid = prop_name_guid[&sp.name].clone();
            let c = Custom {
                guid: guid.clone(),
                parent_guid: parent_of(&sp.base_property, &prop_name_guid),
                abstract_: sp.abstract_,
                name: sp.name.clone(),
                synonyms: sp.synonyms.clone().unwrap_or_default(),
                description: sp.description.clone(),
            };
            r.guid_to_name.insert(guid, sp.name.clone());
            r.properties.insert(sp.name.clone(), c);
        }
        r
    }

    /// Register the i2 geo property hierarchy (Grid Reference → Latitude /
    /// Longitude under Abstract Number), used by latlon geo-map injection so the
    /// Latitude/Longitude attribute classes resolve to the standard GUIDs.
    pub fn register_geo_properties(&mut self) {
        use crate::interop::{
            GRID_REFERENCE_GUID, LATITUDE_GUID, LONGITUDE_GUID, ROOT_ABSTRACT_NUM,
        };
        let mut add = |name: &str, guid: &str, parent: &str| {
            self.guid_to_name.insert(guid.to_string(), name.to_string());
            self.properties.insert(
                name.to_string(),
                Custom {
                    guid: guid.to_string(),
                    parent_guid: Some(parent.to_string()),
                    abstract_: false,
                    name: name.to_string(),
                    synonyms: vec![],
                    description: None,
                },
            );
        };
        add("Grid Reference", GRID_REFERENCE_GUID, ROOT_ABSTRACT_NUM);
        add("Latitude", LATITUDE_GUID, GRID_REFERENCE_GUID);
        add("Longitude", LONGITUDE_GUID, GRID_REFERENCE_GUID);
    }

    /// Resolve an entity/link semantic-type name to its GUID (tracking it as
    /// referenced). `guid…` literals pass through; unknown names are returned
    /// unchanged (validation flags them).
    pub fn resolve_type_name(&mut self, name: Option<&str>) -> Option<String> {
        let name = name?;
        if name.is_empty() {
            return None;
        }
        if name.starts_with("guid") {
            self.referenced_type_guids.insert(name.to_string());
            return Some(name.to_string());
        }
        if let Some(c) = self.entities.get(name).or_else(|| self.links.get(name)) {
            self.referenced_type_guids.insert(c.guid.clone());
            return Some(c.guid.clone());
        }
        Some(name.to_string())
    }

    /// Resolve a property semantic-type name to its GUID.
    pub fn resolve_property_name(&mut self, name: Option<&str>) -> Option<String> {
        let name = name?;
        if name.is_empty() {
            return None;
        }
        if name.starts_with("guid") {
            self.referenced_property_guids.insert(name.to_string());
            return Some(name.to_string());
        }
        if let Some(c) = self.properties.get(name) {
            self.referenced_property_guids.insert(c.guid.clone());
            return Some(c.guid.clone());
        }
        Some(name.to_string())
    }

    fn ancestor_chain(&self, start_guid: &str) -> Vec<String> {
        // Walk parents to the root, then reverse to ancestors-first.
        let mut chain = Vec::new();
        let mut seen = IndexSet::new();
        let mut cur = self.guid_to_name.get(start_guid).cloned();
        while let Some(name) = cur {
            if !seen.insert(name.clone()) {
                break; // circular guard
            }
            chain.push(name.clone());
            let custom = self.entities.get(&name).or_else(|| self.links.get(&name));
            cur = match custom.and_then(|c| c.parent_guid.clone()) {
                Some(pg) => self.guid_to_name.get(&pg).cloned(),
                None => None,
            };
        }
        chain.reverse();
        chain
    }

    fn ancestor_chain_prop(&self, start_guid: &str) -> Vec<String> {
        let mut chain = Vec::new();
        let mut seen = IndexSet::new();
        let mut cur = self.guid_to_name.get(start_guid).cloned();
        while let Some(name) = cur {
            if !seen.insert(name.clone()) {
                break;
            }
            chain.push(name.clone());
            cur = match self
                .properties
                .get(&name)
                .and_then(|c| c.parent_guid.clone())
            {
                Some(pg) => self.guid_to_name.get(&pg).cloned(),
                None => None,
            };
        }
        chain.reverse();
        chain
    }

    fn cat_for(&self, name: &str) -> CatType {
        if let Some(c) = self
            .entities
            .get(name)
            .or_else(|| self.links.get(name))
            .or_else(|| self.properties.get(name))
        {
            CatType {
                guid: c.guid.clone(),
                parent_guid: c.parent_guid.clone(),
                abstract_: c.abstract_,
                name: c.name.clone(),
                synonyms: c.synonyms.clone(),
                description: c.description.clone(),
            }
        } else {
            // A root: look up its guid by name.
            let guid = root_names()
                .into_iter()
                .find(|(n, _)| *n == name)
                .map(|(_, g)| g.to_string())
                .unwrap_or_default();
            CatType {
                guid,
                parent_guid: None,
                abstract_: true,
                name: name.to_string(),
                synonyms: vec![],
                description: None,
            }
        }
    }

    /// Build the catalogue (types, properties) in ancestors-first order, or
    /// `None` when nothing semantic is referenced or defined.
    pub fn build_catalogue(&self) -> Option<(Vec<CatType>, Vec<CatType>)> {
        let has_any = !self.referenced_type_guids.is_empty()
            || !self.referenced_property_guids.is_empty()
            || !self.entities.is_empty()
            || !self.links.is_empty()
            || !self.properties.is_empty();
        if !has_any {
            return None;
        }

        let mut type_names: IndexSet<String> = IndexSet::new();
        for g in &self.referenced_type_guids {
            for n in self.ancestor_chain(g) {
                type_names.insert(n);
            }
        }
        // Include defined customs even if unreferenced.
        for name in self.entities.keys().chain(self.links.keys()) {
            let g = self
                .guid_to_name
                .iter()
                .find(|(_, n)| *n == name)
                .map(|(g, _)| g.clone());
            if let Some(g) = g {
                for n in self.ancestor_chain(&g) {
                    type_names.insert(n);
                }
            }
        }
        let mut prop_names: IndexSet<String> = IndexSet::new();
        for g in &self.referenced_property_guids {
            for n in self.ancestor_chain_prop(g) {
                prop_names.insert(n);
            }
        }
        for name in self.properties.keys() {
            let g = self
                .guid_to_name
                .iter()
                .find(|(_, n)| *n == name)
                .map(|(g, _)| g.clone());
            if let Some(g) = g {
                for n in self.ancestor_chain_prop(&g) {
                    prop_names.insert(n);
                }
            }
        }

        if type_names.is_empty() && prop_names.is_empty() {
            return None;
        }
        let types = type_names.iter().map(|n| self.cat_for(n)).collect();
        let props = prop_names.iter().map(|n| self.cat_for(n)).collect();
        Some((types, props))
    }
}
