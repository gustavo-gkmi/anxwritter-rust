//! Differential output corpus: a broad set of charts whose Python reference
//! output is committed in `tests/fixtures/output_corpus.json`. For each spec we
//! build the chart in Rust and assert the result is *content-identical* to the
//! Python golden — compared as an order-insensitive multiset of canonicalized
//! elements (volatile id numbers + Origin timestamp blanked, attributes sorted),
//! the project's "valid, ordering-tolerant" parity bar.
//!
//! One mechanism exercises every feature: representations, auto-colour, layouts,
//! cards, semantic types, geo, icon-map, link styling, display synthesizers,
//! grades, strengths, summary/legend, enum aliases.

use std::collections::BTreeMap;

use anxwritter::builder::Builder;
use anxwritter::input::{ChartData, Config};
use regex::Regex;

#[derive(serde::Deserialize)]
struct Case {
    name: String,
    spec: serde_json::Value,
    golden: String,
}

/// Canonicalize one element line: blank `ID<n>` ids/refs and `CreatedDate`, then
/// sort attributes so ordering differences don't matter.
fn canon_line(line: &str, id_re: &Regex, cd_re: &Regex, attr_re: &Regex) -> String {
    if line.starts_with("</") || line.starts_with("<?") || line.starts_with("<!--") {
        return line.to_string();
    }
    let s = id_re.replace_all(line, "=\"X\"");
    let s = cd_re.replace_all(&s, "CreatedDate=\"X\"");
    let tag_end = s.find([' ', '>', '/']).unwrap_or(s.len());
    let tag = &s[..tag_end];
    let mut attrs: Vec<&str> = attr_re.find_iter(&s).map(|m| m.as_str()).collect();
    attrs.sort_unstable();
    let suffix = if s.trim_end().ends_with("/>") {
        "/>"
    } else {
        ">"
    };
    format!("{tag} {} {suffix}", attrs.join(" "))
}

fn multiset(xml: &str) -> BTreeMap<String, i64> {
    let id_re = Regex::new(r#"="ID\d+""#).unwrap();
    let cd_re = Regex::new(r#"CreatedDate="[^"]*""#).unwrap();
    let attr_re = Regex::new(r#"[\w:.]+="[^"]*""#).unwrap();
    let mut m = BTreeMap::new();
    for line in xml.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        *m.entry(canon_line(line, &id_re, &cd_re, &attr_re))
            .or_insert(0) += 1;
    }
    m
}

#[test]
fn output_corpus_matches_python_reference() {
    let raw = include_str!("fixtures/output_corpus.json");
    let cases: Vec<Case> = serde_json::from_str(raw).expect("parse corpus fixture");
    assert!(
        cases.len() >= 20,
        "expected a broad corpus, got {}",
        cases.len()
    );

    let mut failures = Vec::new();
    for case in &cases {
        let text = case.spec.to_string();
        let config: Config = serde_json::from_str(&text).unwrap_or_default();
        let data: ChartData = serde_json::from_str(&text).expect("spec parses as ChartData");
        let rust = Builder::new(&config).build(&data);

        let (want, got) = (multiset(&case.golden), multiset(&rust));
        if want != got {
            let only_py: Vec<_> = want
                .iter()
                .filter(|(k, v)| got.get(*k) != Some(*v))
                .take(4)
                .collect();
            let only_ru: Vec<_> = got
                .iter()
                .filter(|(k, v)| want.get(*k) != Some(*v))
                .take(4)
                .collect();
            failures.push(format!(
                "{}:\n      python-only: {:?}\n      rust-only:   {:?}",
                case.name, only_py, only_ru
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "output corpus mismatches ({}):\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}
