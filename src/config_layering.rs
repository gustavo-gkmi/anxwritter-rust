//! Config layering, mirroring `anxwritter/_config_layering.py` (a faithful,
//! somewhat simplified subset).
//!
//! Layers are applied to a merged JSON object in order. Each layer may carry a
//! top-level `cascade: { mode }` block selecting an operation:
//!
//! - `merge` (default): deep-merge settings; upsert keyed sections by `name`;
//!   append list-only sections.
//! - `wipe`:  merge, but clear each mentioned section first.
//! - `delete`: remove the named entries (or whole list-only sections).
//! - `lock`:  merge, then freeze the entries this layer declared — a later layer
//!   trying to change a locked entry records a `locked_override` conflict and
//!   the locked value is kept.
//!
//! The merged object is finally deserialized into [`Config`].

use std::collections::HashMap;

use serde_json::{Map, Value};

use crate::error::{ErrorType, ValidationError};
use crate::input::Config;

/// Sections merged by the `name` identity field.
const KEYED_SECTIONS: &[&str] = &[
    "entity_types",
    "link_types",
    "attribute_classes",
    "datetime_formats",
    "semantic_entities",
    "semantic_links",
    "semantic_properties",
];

/// List-only sections that append (no identity).
const APPEND_SECTIONS: &[&str] = &["legend_items", "palettes", "validators"];

#[derive(Clone, Copy, PartialEq)]
enum Op {
    Merge,
    Wipe,
    Delete,
    Lock,
}

/// Accumulates config layers into a single merged [`Config`].
#[derive(Default)]
pub struct ConfigStack {
    merged: Map<String, Value>,
    /// Locked leaves: `(section, name, dotted_leaf)` -> locked value. A later
    /// layer writing a different value to a locked leaf is rejected.
    locked_leaves: HashMap<(String, String, String), Value>,
    conflicts: Vec<ValidationError>,
}

impl ConfigStack {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a single config layer (a parsed JSON/YAML object).
    pub fn apply(&mut self, mut layer: Value) {
        let op = extract_cascade(&mut layer);
        let Value::Object(obj) = layer else {
            return;
        };
        for (key, val) in obj {
            self.apply_section(&key, val, op);
        }
    }

    fn apply_section(&mut self, key: &str, val: Value, op: Op) {
        if key == "settings" {
            let base = self
                .merged
                .entry("settings")
                .or_insert(Value::Object(Map::new()));
            deep_merge(base, val);
        } else if KEYED_SECTIONS.contains(&key) {
            self.apply_keyed(key, val, op);
        } else if APPEND_SECTIONS.contains(&key) || key == "source_types" {
            self.apply_list(key, val, op);
        } else {
            // strengths, grades_*, and any scalar: delete clears, else replace/merge.
            match op {
                Op::Delete => {
                    self.merged.remove(key);
                }
                _ => {
                    let base = self.merged.entry(key.to_string()).or_insert(Value::Null);
                    if base.is_object() && val.is_object() {
                        deep_merge(base, val);
                    } else {
                        *base = val;
                    }
                }
            }
        }
    }

    fn apply_keyed(&mut self, section: &str, val: Value, op: Op) {
        let Value::Array(incoming) = val else { return };
        if op == Op::Wipe {
            self.merged
                .insert(section.to_string(), Value::Array(vec![]));
        }
        let arr = match self
            .merged
            .entry(section.to_string())
            .or_insert(Value::Array(vec![]))
        {
            Value::Array(a) => a,
            other => {
                *other = Value::Array(vec![]);
                let Value::Array(a) = other else {
                    unreachable!()
                };
                a
            }
        };
        // Collect operations first to avoid borrowing self.merged + self.locked
        // simultaneously.
        let mut conflicts = Vec::new();
        let mut new_locks: Vec<((String, String, String), Value)> = Vec::new();
        for entry in incoming {
            let name = entry
                .get("name")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let Some(name) = name else {
                arr.push(entry);
                continue;
            };
            let pos = arr
                .iter()
                .position(|e| e.get("name").and_then(|v| v.as_str()) == Some(&name));
            match op {
                Op::Delete => {
                    delete_keyed_entry(arr, pos, &entry, section, &name, &mut conflicts);
                }
                _ => {
                    if pos.is_none() {
                        arr.push(serde_json::json!({ "name": name }));
                    }
                    let p = pos.unwrap_or(arr.len() - 1);
                    // Apply each leaf, honouring existing leaf locks.
                    for (leaf, lval) in walk_leaves(&entry, "") {
                        if leaf == "name" {
                            continue;
                        }
                        let lk = (section.to_string(), name.clone(), leaf.clone());
                        if let Some(locked) = self.locked_leaves.get(&lk) {
                            if locked != &lval {
                                conflicts.push(ValidationError::new(
                                    ErrorType::LockedOverride,
                                    format!("{section}.{name}.{leaf}"),
                                    format!("attempt to change locked '{leaf}' on '{name}'"),
                                ));
                                continue;
                            }
                        }
                        set_leaf(&mut arr[p], &leaf, lval.clone());
                        if op == Op::Lock {
                            new_locks.push((lk, lval));
                        }
                    }
                }
            }
        }
        self.conflicts.extend(conflicts);
        for (k, v) in new_locks {
            self.locked_leaves.insert(k, v);
        }
    }

    fn apply_list(&mut self, section: &str, val: Value, op: Op) {
        let Value::Array(incoming) = val else { return };
        if matches!(op, Op::Wipe | Op::Delete) {
            self.merged
                .insert(section.to_string(), Value::Array(vec![]));
            if op == Op::Delete {
                return;
            }
        }
        let arr = match self
            .merged
            .entry(section.to_string())
            .or_insert(Value::Array(vec![]))
        {
            Value::Array(a) => a,
            other => {
                *other = Value::Array(vec![]);
                let Value::Array(a) = other else {
                    unreachable!()
                };
                a
            }
        };
        for item in incoming {
            // source_types dedups by value.
            if section == "source_types" && arr.contains(&item) {
                continue;
            }
            arr.push(item);
        }
    }

    /// Deserialize the merged object into a [`Config`], returning any layering
    /// conflicts collected along the way.
    pub fn finish(self) -> (Config, Vec<ValidationError>) {
        let config = serde_json::from_value(Value::Object(self.merged)).unwrap_or_default();
        (config, self.conflicts)
    }
}

/// Pop the top-level `cascade: { mode }` block and map it to an operation.
fn extract_cascade(layer: &mut Value) -> Op {
    let Value::Object(obj) = layer else {
        return Op::Merge;
    };
    let Some(cascade) = obj.remove("cascade") else {
        return Op::Merge;
    };
    match cascade.get("mode").and_then(|v| v.as_str()) {
        Some("wipe") => Op::Wipe,
        Some("delete") => Op::Delete,
        Some("lock") => Op::Lock,
        _ => Op::Merge,
    }
}

/// Flatten an object into `(dotted_path, leaf_value)` pairs (objects recurse,
/// scalars and arrays are leaves).
fn walk_leaves(v: &Value, prefix: &str) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    match v {
        Value::Object(m) => {
            for (k, val) in m {
                let p = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                out.extend(walk_leaves(val, &p));
            }
        }
        other => out.push((prefix.to_string(), other.clone())),
    }
    out
}

/// Set a leaf at a dotted path inside an object, creating intermediate objects.
fn set_leaf(obj: &mut Value, dotted: &str, value: Value) {
    let parts: Vec<&str> = dotted.split('.').collect();
    let mut cur = obj;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            if let Value::Object(m) = cur {
                m.insert(part.to_string(), value);
            }
            return;
        }
        if !cur.is_object() {
            *cur = Value::Object(Map::new());
        }
        cur = cur
            .as_object_mut()
            .unwrap()
            .entry(part.to_string())
            .or_insert(Value::Object(Map::new()));
    }
}

/// Delete a keyed entry: whole-entry when only the identity is given, otherwise
/// field-level (each named non-identity field must be null and is reset).
fn delete_keyed_entry(
    arr: &mut Vec<Value>,
    pos: Option<usize>,
    entry: &Value,
    section: &str,
    name: &str,
    conflicts: &mut Vec<ValidationError>,
) {
    let named: Vec<(String, Value)> = entry
        .as_object()
        .map(|m| {
            m.iter()
                .filter(|(k, _)| *k != "name")
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default();
    let Some(p) = pos else { return };
    if named.is_empty() {
        arr.remove(p);
        return;
    }
    for (k, v) in named {
        if !v.is_null() {
            conflicts.push(ValidationError::new(
                ErrorType::DeleteContract,
                format!("{section}.{name}.{k}"),
                "delete entries must set non-identity fields to null",
            ));
            continue;
        }
        if let Value::Object(m) = &mut arr[p] {
            m.remove(&k); // resets the field to its default
        }
    }
}

/// Recursive deep merge: dict keys recurse, scalars and arrays replace.
fn deep_merge(base: &mut Value, incoming: Value) {
    match (base, incoming) {
        (Value::Object(b), Value::Object(i)) => {
            for (k, v) in i {
                match b.get_mut(&k) {
                    Some(slot) if slot.is_object() && v.is_object() => deep_merge(slot, v),
                    _ => {
                        b.insert(k, v);
                    }
                }
            }
        }
        (base, incoming) => *base = incoming,
    }
}

/// Convenience: layer several JSON config values, then a final merged config.
pub fn merge_configs(layers: Vec<Value>) -> (Config, Vec<ValidationError>) {
    let mut stack = ConfigStack::new();
    for l in layers {
        stack.apply(l);
    }
    stack.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deep_merges_settings_across_layers() {
        let (cfg, _) = merge_configs(vec![
            json!({"settings": {"chart": {"rigorous": true}, "view": {"time_bar": true}}}),
            json!({"settings": {"chart": {"bg_filled": true}}}),
        ]);
        let s = cfg.settings.unwrap();
        assert_eq!(s.chart.rigorous, Some(true)); // preserved from layer 1
        assert_eq!(s.chart.bg_filled, Some(true)); // added by layer 2
        assert_eq!(s.view.time_bar, Some(true));
    }

    #[test]
    fn keyed_upsert_and_delete_by_name() {
        let (cfg, _) = merge_configs(vec![
            json!({"entity_types": [{"name": "Person", "icon_file": "adult"}, {"name": "Car"}]}),
            json!({"entity_types": [{"name": "Person", "color": 255}]}),
            json!({"cascade": {"mode": "delete"}, "entity_types": [{"name": "Car"}]}),
        ]);
        assert_eq!(cfg.entity_types.len(), 1);
        let p = &cfg.entity_types[0];
        assert_eq!(p.name, "Person");
        assert_eq!(p.icon_file.as_deref(), Some("adult")); // preserved
        assert!(p.color.is_some()); // merged in
    }

    #[test]
    fn wipe_clears_section_first() {
        let (cfg, _) = merge_configs(vec![
            json!({"source_types": ["A", "B"]}),
            json!({"cascade": {"mode": "wipe"}, "source_types": ["C"]}),
        ]);
        assert_eq!(cfg.source_types, vec!["C".to_string()]);
    }

    #[test]
    fn lock_blocks_later_override_and_records_conflict() {
        let (cfg, conflicts) = merge_configs(vec![
            json!({"cascade": {"mode": "lock"}, "link_types": [{"name": "Calls", "color": 1}]}),
            json!({"link_types": [{"name": "Calls", "color": 999}]}),
        ]);
        assert_eq!(
            cfg.link_types[0]
                .color
                .as_ref()
                .unwrap()
                .to_colorref()
                .unwrap(),
            1
        );
        assert!(conflicts
            .iter()
            .any(|e| e.error_type == ErrorType::LockedOverride));
    }

    #[test]
    fn leaf_lock_only_freezes_locked_field() {
        let (cfg, conflicts) = merge_configs(vec![
            json!({"cascade": {"mode": "lock"}, "entity_types": [{"name": "Person", "color": 1}]}),
            json!({"entity_types": [{"name": "Person", "color": 999, "icon_file": "adult"}]}),
        ]);
        let p = &cfg.entity_types[0];
        assert_eq!(p.color.as_ref().unwrap().to_colorref().unwrap(), 1); // locked
        assert_eq!(p.icon_file.as_deref(), Some("adult")); // not locked -> applied
        assert!(conflicts
            .iter()
            .any(|e| e.error_type == ErrorType::LockedOverride));
    }

    #[test]
    fn field_level_delete_resets_only_named_field() {
        let (cfg, conflicts) = merge_configs(vec![
            json!({"entity_types": [{"name": "Person", "icon_file": "adult", "color": 128}]}),
            json!({"cascade": {"mode": "delete"}, "entity_types": [{"name": "Person", "color": null}]}),
        ]);
        let p = &cfg.entity_types[0];
        assert_eq!(p.name, "Person");
        assert_eq!(p.icon_file.as_deref(), Some("adult")); // preserved
        assert!(p.color.is_none()); // reset to default
        assert!(conflicts.is_empty());
    }

    #[test]
    fn field_delete_with_nonnull_value_is_a_contract_error() {
        let (_cfg, conflicts) = merge_configs(vec![
            json!({"entity_types": [{"name": "Person", "color": 128}]}),
            json!({"cascade": {"mode": "delete"}, "entity_types": [{"name": "Person", "color": 5}]}),
        ]);
        assert!(conflicts
            .iter()
            .any(|e| e.error_type == ErrorType::DeleteContract));
    }
}
