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

/// The layering operation applied to a config layer.
///
/// Mirrors the Python `cascade.mode` vocabulary and the
/// `apply_config(operation=, wipe_previous=, lock=)` kwargs table one-to-one —
/// the server maps a request-level mode string to exactly these four cases:
///
/// | `CascadeMode` | Python `operation` / `wipe_previous` / `lock` |
/// |---------------|-----------------------------------------------|
/// | [`Merge`](CascadeMode::Merge)   | `merge`  / `false` / `false` |
/// | [`Wipe`](CascadeMode::Wipe)     | `merge`  / `true`  / `false` |
/// | [`Delete`](CascadeMode::Delete) | `delete` / `false` / `false` |
/// | [`Lock`](CascadeMode::Lock)     | `merge`  / `false` / `true`  |
///
/// Pass one to [`ConfigStack::apply_with`] to override a layer's own embedded
/// `cascade.mode` for that call (the equivalent of the Python kwargs). Passing
/// `None` there honours the layer's embedded `cascade`, defaulting to `Merge`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CascadeMode {
    /// Deep-merge settings; upsert keyed sections by `name`; append list-only
    /// sections. The default when no cascade is set.
    Merge,
    /// Merge, but clear each mentioned section first ("narrow the list").
    Wipe,
    /// Remove the named entries, or clear whole list-only sections.
    Delete,
    /// Merge, then freeze the leaves this layer declared — a later layer trying
    /// to change a locked leaf records a `locked_override` conflict and the
    /// locked value is kept.
    Lock,
}

#[derive(Clone, Copy, PartialEq)]
enum Op {
    Merge,
    Wipe,
    Delete,
    Lock,
}

impl From<CascadeMode> for Op {
    fn from(m: CascadeMode) -> Self {
        match m {
            CascadeMode::Merge => Op::Merge,
            CascadeMode::Wipe => Op::Wipe,
            CascadeMode::Delete => Op::Delete,
            CascadeMode::Lock => Op::Lock,
        }
    }
}

/// A locked leaf's value plus the `source` label of the layer that locked it.
type LockedLeaf = (Value, Option<String>);

/// Accumulates config layers into a single merged [`Config`].
#[derive(Default)]
pub struct ConfigStack {
    merged: Map<String, Value>,
    /// Locked leaves: `(section, name, dotted_leaf)` -> `(locked value, source)`.
    /// A later layer writing a different value to a locked leaf is rejected; the
    /// recorded `source` tags the resulting conflict's `config_source`.
    locked_leaves: HashMap<(String, String, String), LockedLeaf>,
    conflicts: Vec<ValidationError>,
}

impl ConfigStack {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a single config layer (a parsed JSON/YAML object), honouring its
    /// embedded `cascade.mode` (defaulting to merge). Equivalent to
    /// [`apply_with`](Self::apply_with)`(layer, None, None)`.
    pub fn apply(&mut self, layer: Value) {
        self.apply_with(layer, None, None);
    }

    /// Apply a single config layer, optionally overriding its embedded
    /// `cascade.mode` and tagging its provenance.
    ///
    /// - `mode`: when `Some`, overrides the layer's own `cascade.mode` for this
    ///   call (mirrors the Python `apply_config` `operation`/`wipe_previous`/
    ///   `lock` kwargs — see [`CascadeMode`]). When `None`, the layer's embedded
    ///   `cascade` drives the operation, defaulting to [`CascadeMode::Merge`].
    /// - `source`: a provenance label (e.g. a standard/file name) for this
    ///   layer. It is carried onto any [`ValidationError`] this layer produces
    ///   (`locked_override`, `delete_contract`) as the error's `source`, and is
    ///   recorded on the leaves a `Lock` layer freezes so a later override
    ///   conflict can name the locking layer via `config_source`.
    ///
    /// The embedded `cascade` block is always stripped from the layer, whether
    /// or not `mode` overrides it.
    pub fn apply_with(
        &mut self,
        mut layer: Value,
        mode: Option<CascadeMode>,
        source: Option<&str>,
    ) {
        let embedded = extract_cascade(&mut layer);
        let op = mode.map(Op::from).unwrap_or(embedded);
        let Value::Object(obj) = layer else {
            return;
        };
        for (key, val) in obj {
            self.apply_section(&key, val, op, source);
        }
    }

    fn apply_section(&mut self, key: &str, val: Value, op: Op, source: Option<&str>) {
        if key == "settings" {
            let base = self
                .merged
                .entry("settings")
                .or_insert(Value::Object(Map::new()));
            deep_merge(base, val);
        } else if KEYED_SECTIONS.contains(&key) {
            self.apply_keyed(key, val, op, source);
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

    fn apply_keyed(&mut self, section: &str, val: Value, op: Op, source: Option<&str>) {
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
        let mut new_locks: Vec<((String, String, String), LockedLeaf)> = Vec::new();
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
                    delete_keyed_entry(arr, pos, &entry, section, &name, source, &mut conflicts);
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
                        if let Some((locked, lock_src)) = self.locked_leaves.get(&lk) {
                            if locked != &lval {
                                let mut err = ValidationError::new(
                                    ErrorType::LockedOverride,
                                    format!("{section}.{name}.{leaf}"),
                                    format!("attempt to change locked '{leaf}' on '{name}'"),
                                );
                                err.source = source.map(str::to_string);
                                err.config_source = lock_src.clone();
                                conflicts.push(err);
                                continue;
                            }
                        }
                        set_leaf(&mut arr[p], &leaf, lval.clone());
                        if op == Op::Lock {
                            new_locks.push((lk, (lval, source.map(str::to_string))));
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
    source: Option<&str>,
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
            let mut err = ValidationError::new(
                ErrorType::DeleteContract,
                format!("{section}.{name}.{k}"),
                "delete entries must set non-identity fields to null",
            );
            err.source = source.map(str::to_string);
            conflicts.push(err);
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

    // ── per-call mode override (gap 1) ──────────────────────────────────────

    #[test]
    fn mode_override_beats_embedded_cascade() {
        // The layer carries no cascade (would merge), but the caller overrides
        // with Delete — the Car entry must be removed.
        let mut stack = ConfigStack::new();
        stack.apply(json!({"entity_types": [{"name": "Person"}, {"name": "Car"}]}));
        stack.apply_with(
            json!({"entity_types": [{"name": "Car"}]}),
            Some(CascadeMode::Delete),
            None,
        );
        let (cfg, _) = stack.finish();
        let names: Vec<&str> = cfg.entity_types.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["Person"]);
    }

    #[test]
    fn mode_override_none_honours_embedded_cascade() {
        // mode = None must preserve the layer's own cascade (here: wipe).
        let mut stack = ConfigStack::new();
        stack.apply(json!({"source_types": ["A", "B"]}));
        stack.apply_with(
            json!({"cascade": {"mode": "wipe"}, "source_types": ["C"]}),
            None,
            None,
        );
        let (cfg, _) = stack.finish();
        assert_eq!(cfg.source_types, vec!["C".to_string()]);
    }

    #[test]
    fn mode_override_can_downgrade_embedded_lock_to_merge() {
        // Layer says lock, but the caller overrides to plain merge, so a later
        // layer freely changes the value with no conflict.
        let mut stack = ConfigStack::new();
        stack.apply_with(
            json!({"cascade": {"mode": "lock"}, "link_types": [{"name": "Calls", "color": 1}]}),
            Some(CascadeMode::Merge),
            None,
        );
        stack.apply(json!({"link_types": [{"name": "Calls", "color": 999}]}));
        let (cfg, conflicts) = stack.finish();
        assert_eq!(
            cfg.link_types[0]
                .color
                .as_ref()
                .unwrap()
                .to_colorref()
                .unwrap(),
            999
        );
        assert!(conflicts.is_empty());
    }

    #[test]
    fn cascade_modes_match_python_kwargs_table() {
        // Wipe clears then merges; Delete removes; Lock freezes + conflicts.
        let wipe = {
            let mut s = ConfigStack::new();
            s.apply(json!({"source_types": ["A", "B"]}));
            s.apply_with(
                json!({"source_types": ["C"]}),
                Some(CascadeMode::Wipe),
                None,
            );
            s.finish().0.source_types
        };
        assert_eq!(wipe, vec!["C".to_string()]);

        let (lock_cfg, lock_conflicts) = {
            let mut s = ConfigStack::new();
            s.apply_with(
                json!({"entity_types": [{"name": "P", "color": 1}]}),
                Some(CascadeMode::Lock),
                None,
            );
            s.apply_with(
                json!({"entity_types": [{"name": "P", "color": 2}]}),
                Some(CascadeMode::Merge),
                None,
            );
            s.finish()
        };
        assert_eq!(
            lock_cfg.entity_types[0]
                .color
                .as_ref()
                .unwrap()
                .to_colorref()
                .unwrap(),
            1
        );
        assert!(lock_conflicts
            .iter()
            .any(|e| e.error_type == ErrorType::LockedOverride));
    }

    // ── source provenance on conflicts (gap 2) ──────────────────────────────

    #[test]
    fn locked_override_conflict_carries_both_sources() {
        // The locking layer's source ("org-base/090@lock") must surface as
        // `config_source`; the overriding layer's source as `source`.
        let mut stack = ConfigStack::new();
        stack.apply_with(
            json!({"link_types": [{"name": "Calls", "color": 1}]}),
            Some(CascadeMode::Lock),
            Some("org-base/090@lock"),
        );
        stack.apply_with(
            json!({"link_types": [{"name": "Calls", "color": 999}]}),
            None,
            Some("tenant-override"),
        );
        let (_cfg, conflicts) = stack.finish();
        let c = conflicts
            .iter()
            .find(|e| e.error_type == ErrorType::LockedOverride)
            .expect("locked_override conflict");
        assert_eq!(c.config_source.as_deref(), Some("org-base/090@lock"));
        assert_eq!(c.source.as_deref(), Some("tenant-override"));
        assert_eq!(c.location, "link_types.Calls.color");
    }

    #[test]
    fn delete_contract_conflict_carries_source() {
        let mut stack = ConfigStack::new();
        stack.apply(json!({"entity_types": [{"name": "Person", "color": 128}]}));
        stack.apply_with(
            json!({"entity_types": [{"name": "Person", "color": 5}]}),
            Some(CascadeMode::Delete),
            Some("bad-delete-layer"),
        );
        let (_cfg, conflicts) = stack.finish();
        let c = conflicts
            .iter()
            .find(|e| e.error_type == ErrorType::DeleteContract)
            .expect("delete_contract conflict");
        assert_eq!(c.source.as_deref(), Some("bad-delete-layer"));
    }
}
