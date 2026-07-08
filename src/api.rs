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

    /// Build a config layer from an already-parsed [`serde_json::Value`], without
    /// a serialize-then-reparse round-trip. Useful for a server that parses the
    /// request body once (to split config/data or accept YAML) and then feeds the
    /// pieces straight in.
    pub fn from_value(v: serde_json::Value) -> Result<Self> {
        Ok(serde_json::from_value(v)?)
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

    /// Build a data layer from an already-parsed [`serde_json::Value`], without a
    /// serialize-then-reparse round-trip. See [`Config::from_value`].
    pub fn from_value(v: serde_json::Value) -> Result<Self> {
        Ok(serde_json::from_value(v)?)
    }
}

/// Build a chart to its compact XML string **without validating** — the
/// low-level primitive (equivalent to Python's `_build_xml`). The returned string
/// is UTF-8 and declares `encoding='utf-8'`; the `.anx` byte APIs render the
/// UTF-16-declared form. Use [`to_xml`] or [`build_anx`] for the validating,
/// public-facing path.
pub fn render_xml(config: &Config, data: &ChartData) -> String {
    Builder::new(config).build(data)
}

/// Render the compact, UTF-16-declared XML string that backs the `.anx` byte
/// forms (before UTF-16 LE encoding). Internal to the byte-producing helpers.
fn render_anx_xml(config: &Config, data: &ChartData) -> String {
    Builder::new(config).build_with_encoding(data, true, "utf-16")
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
    let bytes = to_utf16le_with_bom(&render_anx_xml(config, data));
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
///
/// The trailing `bool` is `validate` — note that [`Builder::write_to`] takes a
/// `compact` bool in the same position (opposite meaning). Prefer
/// [`write_anx_with`] with [`BuildOptions`] to name the flag at the call site.
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
    let xml = render_anx_xml(config, data);
    Ok(to_utf16le_with_bom(&xml))
}

/// Options for the `.anx` build entry points, replacing the bare trailing
/// `bool`. The free functions ([`build_anx`] / [`write_anx`]) take a `validate`
/// bool while [`Builder::build_with`] / [`Builder::write_to`] take a `compact`
/// bool in the same position — opposite meanings that are easy to confuse.
/// [`build_anx_with`] / [`write_anx_with`] take this struct so each flag is named
/// at the call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildOptions {
    /// Emit the compact (newline-separated, unindented) form — the `.anx` file
    /// default. `false` produces the pretty, indented inspection form.
    pub compact: bool,
    /// Validate before building, returning [`Error::Validation`] on any error.
    pub validate: bool,
}

impl Default for BuildOptions {
    /// Compact and validating — the safe, ANB-ready default.
    fn default() -> Self {
        BuildOptions {
            compact: true,
            validate: true,
        }
    }
}

/// Like [`build_anx`], but every flag is named via [`BuildOptions`] (no
/// ambiguous trailing `bool`). Materializes the `.anx` bytes (UTF-16 LE + BOM).
pub fn build_anx_with(config: &Config, data: &ChartData, opts: BuildOptions) -> Result<Vec<u8>> {
    if opts.validate {
        validate_or_err(config, data)?;
    }
    let xml = Builder::new(config).build_with_encoding(data, opts.compact, "utf-16");
    Ok(to_utf16le_with_bom(&xml))
}

/// Like [`write_anx`], but every flag is named via [`BuildOptions`]. Streams the
/// `.anx` bytes (UTF-16 LE + BOM) into `w` with bounded peak memory.
pub fn write_anx_with<W: std::io::Write>(
    config: &Config,
    data: &ChartData,
    w: W,
    opts: BuildOptions,
) -> Result<()> {
    if opts.validate {
        validate_or_err(config, data)?;
    }
    Builder::new(config).write_to(data, w, opts.compact)?;
    Ok(())
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

    fn sample() -> ChartData {
        ChartData::from_json(r#"{"entities":{"icons":[{"id":"a","type":"P"}]},"links":[]}"#)
            .unwrap()
    }

    /// The declaration must name the encoding of the bytes each form hands back:
    /// `utf-8` for the string forms, `utf-16` for the `.anx` byte forms (upstream
    /// 1.25.0). The `.anx` on-disk bytes stay UTF-16 LE.
    #[test]
    fn declaration_encoding_matches_output_form() {
        let cfg = Config::default();
        let data = sample();

        // String forms → utf-8.
        assert!(to_xml(&cfg, &data)
            .unwrap()
            .starts_with("<?xml version='1.0' encoding='utf-8'?>"));
        assert!(render_xml(&cfg, &data).starts_with("<?xml version='1.0' encoding='utf-8'?>"));

        // Streaming UTF-8 XML → utf-8, and the bytes really are UTF-8 (no BOM).
        let mut buf = Vec::new();
        Builder::new(&cfg)
            .write_xml_to(&data, &mut buf, true)
            .unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.starts_with("<?xml version='1.0' encoding='utf-8'?>"));

        // .anx byte forms → utf-16 declaration, UTF-16 LE + BOM on disk.
        let bytes = build_anx(&cfg, &data, true).unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xFE]);
        let units: Vec<u16> = bytes[2..]
            .chunks(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let text = String::from_utf16(&units).unwrap();
        assert!(text.starts_with("<?xml version='1.0' encoding='utf-16'?>"));
    }

    #[test]
    fn from_value_matches_from_json() {
        let json = r#"{"entities":{"icons":[{"id":"a","type":"P"}]},"links":[]}"#;
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        let a = build_anx(&Config::default(), &ChartData::from_value(v).unwrap(), true).unwrap();
        let b = build_anx(
            &Config::default(),
            &ChartData::from_json(json).unwrap(),
            true,
        )
        .unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn build_options_default_is_compact_and_validating() {
        let opts = BuildOptions::default();
        assert!(opts.compact && opts.validate);
        let cfg = Config::default();
        let data = sample();
        assert_eq!(
            build_anx_with(&cfg, &data, opts).unwrap(),
            build_anx(&cfg, &data, true).unwrap()
        );
    }
}
