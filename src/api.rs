//! High-level convenience API: parse → validate → build → UTF-16 `.anx` bytes.
//!
//! This is the surface a server or CLI drives. [`Config`] and [`ChartData`] gain
//! `from_json`/`from_yaml` constructors, and [`build_anx`] runs the whole
//! pipeline, returning either the on-disk `.anx` bytes or the accumulated
//! validation errors.

use crate::builder::Builder;
use crate::error::{AnxValidationError, Error, Result};
use crate::input::{ChartData, Config};
use crate::validation;
use crate::xml::to_utf16le_with_bom;

impl Config {
    /// Parse a config layer from JSON.
    pub fn from_json(s: &str) -> Result<Self> {
        Ok(serde_json::from_str(s)?)
    }

    /// Parse a config layer from YAML.
    pub fn from_yaml(s: &str) -> Result<Self> {
        Ok(serde_yaml_ng::from_str(s)?)
    }

    /// Overlay another config layer on top of this one (a simplified layering:
    /// keyed lists upsert by name, others append, scalars/settings last-wins).
    ///
    /// Full `cascade`/lock/delete/wipe semantics are not yet modelled; this
    /// covers the common "base config + overrides" case.
    pub fn overlay(&mut self, other: Config) {
        if other.settings.is_some() {
            self.settings = other.settings;
        }
        upsert_by_name(&mut self.entity_types, other.entity_types, |e| {
            e.name.clone()
        });
        upsert_by_name(&mut self.link_types, other.link_types, |e| e.name.clone());
        upsert_by_name(&mut self.attribute_classes, other.attribute_classes, |e| {
            e.name.clone()
        });
        upsert_by_name(&mut self.datetime_formats, other.datetime_formats, |e| {
            e.name.clone()
        });
        if other.strengths.is_some() {
            self.strengths = other.strengths;
        }
        if other.grades_one.is_some() {
            self.grades_one = other.grades_one;
        }
        if other.grades_two.is_some() {
            self.grades_two = other.grades_two;
        }
        if other.grades_three.is_some() {
            self.grades_three = other.grades_three;
        }
        self.source_types.extend(other.source_types);
        self.legend_items.extend(other.legend_items);
        self.palettes.extend(other.palettes);
        self.semantic_entities.extend(other.semantic_entities);
        self.semantic_links.extend(other.semantic_links);
        self.semantic_properties.extend(other.semantic_properties);
        self.validators.extend(other.validators);
    }
}

fn upsert_by_name<T>(base: &mut Vec<T>, incoming: Vec<T>, key: impl Fn(&T) -> String) {
    for item in incoming {
        let k = key(&item);
        if let Some(slot) = base.iter_mut().find(|x| key(x) == k) {
            *slot = item;
        } else {
            base.push(item);
        }
    }
}

impl ChartData {
    /// Parse a data layer from JSON.
    pub fn from_json(s: &str) -> Result<Self> {
        Ok(serde_json::from_str(s)?)
    }

    /// Parse a data layer from YAML.
    pub fn from_yaml(s: &str) -> Result<Self> {
        Ok(serde_yaml_ng::from_str(s)?)
    }
}

/// Build a chart to its compact `.anx` XML string **without validating** — the
/// low-level primitive (equivalent to Python's `_build_xml`). Use [`to_xml`] or
/// [`build_anx`] for the validating, public-facing path.
pub fn render_xml(config: &Config, data: &ChartData) -> String {
    Builder::new(config).build(data)
}

/// Validate, then build the chart to a compact XML string — mirrors Python's
/// `to_xml()`, which since anxwritter 1.24.2 defaults to `compact=True` (matching
/// `to_anx`/`iter_xml`). For the pretty/indented inspection form, use
/// [`Builder::build_with(data, false)`](Builder::build_with).
///
/// Returns [`Error::Validation`] with every accumulated error if invalid.
pub fn to_xml(config: &Config, data: &ChartData) -> Result<String> {
    validate_or_err(config, data)?;
    Ok(Builder::new(config).build_with(data, true))
}

fn validate_or_err(config: &Config, data: &ChartData) -> Result<()> {
    let errors = validation::validate(config, data);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(Error::Validation(AnxValidationError::new(errors)))
    }
}

/// Iterator over UTF-16 LE `.anx` bytes (BOM first), yielded in fixed-size
/// chunks. Useful for streaming a chart into an HTTP response body without
/// holding the whole encoded buffer per write.
pub struct AnxByteChunks {
    bytes: Vec<u8>,
    pos: usize,
    chunk: usize,
}

impl Iterator for AnxByteChunks {
    type Item = Vec<u8>;
    fn next(&mut self) -> Option<Vec<u8>> {
        if self.pos >= self.bytes.len() {
            return None;
        }
        // Keep UTF-16 code units intact by chunking on even byte boundaries.
        let end = (self.pos + self.chunk).min(self.bytes.len());
        let end = if end < self.bytes.len() && (end - self.pos) % 2 == 1 {
            end - 1
        } else {
            end
        };
        let out = self.bytes[self.pos..end].to_vec();
        self.pos = end;
        Some(out)
    }
}

/// Validate, then stream a chart's on-disk `.anx` bytes in `chunk` byte pieces —
/// mirrors Python's `iter_anx_bytes()` (which validates first).
pub fn iter_anx_bytes(config: &Config, data: &ChartData, chunk: usize) -> Result<AnxByteChunks> {
    validate_or_err(config, data)?;
    let bytes = to_utf16le_with_bom(&render_xml(config, data));
    Ok(AnxByteChunks {
        bytes,
        pos: 0,
        chunk: chunk.max(2),
    })
}

/// Validate, then **stream** the chart's `.anx` bytes (UTF-16 LE + BOM) directly
/// to `w` with bounded peak memory — never materializing the full document.
/// Mirrors Python's `to_anx(stream=True)`. The preferred path for large charts
/// and HTTP response bodies. Wrap `w` in a [`std::io::BufWriter`] for best I/O.
pub fn write_anx<W: std::io::Write>(
    config: &Config,
    data: &ChartData,
    w: W,
    validate: bool,
) -> Result<()> {
    if validate {
        validate_or_err(config, data)?;
    }
    Builder::new(config).write_to(data, w, true)?;
    Ok(())
}

/// Validate, then build the chart to on-disk `.anx` bytes (UTF-16 LE + BOM),
/// materialized in memory. Prefer [`write_anx`] to stream into a sink instead.
///
/// Returns [`Error::Validation`] with every accumulated error if the chart is
/// invalid and `validate` is true.
pub fn build_anx(config: &Config, data: &ChartData, validate: bool) -> Result<Vec<u8>> {
    if validate {
        let errors = validation::validate(config, data);
        if !errors.is_empty() {
            return Err(Error::Validation(AnxValidationError::new(errors)));
        }
    }
    let xml = render_xml(config, data);
    Ok(to_utf16le_with_bom(&xml))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn end_to_end_json() {
        let data = ChartData::from_json(
            r#"{"entities":{"icons":[{"id":"a","type":"P"},{"id":"b","type":"P"}]},
                "links":[{"from_id":"a","to_id":"b","type":"L"}]}"#,
        )
        .unwrap();
        let bytes = build_anx(&Config::default(), &data, true).unwrap();
        // UTF-16 LE BOM.
        assert_eq!(&bytes[..2], &[0xFF, 0xFE]);
        let units: Vec<u16> = bytes[2..]
            .chunks(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let text = String::from_utf16(&units).unwrap();
        assert!(text.contains("<Chart>"));
        assert!(text.contains("Identity=\"a\""));
    }

    #[test]
    fn validation_blocks_build() {
        let data = ChartData::from_json(
            r#"{"entities":{"icons":[{"id":"a","type":"P"}]},"links":[{"from_id":"a","to_id":"x"}]}"#,
        )
        .unwrap();
        let e = build_anx(&Config::default(), &data, true).unwrap_err();
        assert!(matches!(e, Error::Validation(_)));
    }
}
