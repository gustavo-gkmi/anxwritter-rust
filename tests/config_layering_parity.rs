//! Config-layering parity: apply the same ordered config layers through Rust's
//! `ConfigStack` (honouring each layer's `cascade.mode`: merge/wipe/delete/lock)
//! and assert the built chart matches the Python reference, which applied the
//! same layers via `apply_config`. Compared as a canonicalized element multiset.

use std::collections::BTreeMap;

use anxwritter::builder::Builder;
use anxwritter::config_layering::ConfigStack;
use anxwritter::input::ChartData;
use regex::Regex;

#[derive(serde::Deserialize)]
struct Case {
    name: String,
    layers: Vec<serde_json::Value>,
    data: serde_json::Value,
    golden: String,
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
        let norm = if line.starts_with("</") || line.starts_with("<?") || line.starts_with("<!--") {
            line.to_string()
        } else {
            let s = id_re.replace_all(line, "=\"X\"");
            let s = cd_re.replace_all(&s, "CreatedDate=\"X\"");
            let tag_end = s.find([' ', '>', '/']).unwrap_or(s.len());
            let mut attrs: Vec<&str> = attr_re.find_iter(&s).map(|m| m.as_str()).collect();
            attrs.sort_unstable();
            let suffix = if s.trim_end().ends_with("/>") {
                "/>"
            } else {
                ">"
            };
            format!("{} {} {suffix}", &s[..tag_end], attrs.join(" "))
        };
        *m.entry(norm).or_insert(0) += 1;
    }
    m
}

#[test]
fn config_layering_matches_python_reference() {
    let raw = include_str!("fixtures/config_layering.json");
    let cases: Vec<Case> = serde_json::from_str(raw).expect("parse layering fixture");
    let mut failures = Vec::new();
    for case in &cases {
        let mut stack = ConfigStack::new();
        for layer in &case.layers {
            stack.apply(layer.clone());
        }
        let (config, _conflicts) = stack.finish();
        let data: ChartData = serde_json::from_value(case.data.clone()).unwrap();
        let rust = Builder::new(&config).build(&data);

        let (want, got) = (multiset(&case.golden), multiset(&rust));
        if want != got {
            let only_py: Vec<_> = want
                .iter()
                .filter(|(k, v)| got.get(*k) != Some(*v))
                .take(5)
                .collect();
            let only_ru: Vec<_> = got
                .iter()
                .filter(|(k, v)| want.get(*k) != Some(*v))
                .take(5)
                .collect();
            failures.push(format!(
                "{}:\n      python-only: {:?}\n      rust-only:   {:?}",
                case.name, only_py, only_ru
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "config-layering mismatches:\n  {}",
        failures.join("\n  ")
    );
}
