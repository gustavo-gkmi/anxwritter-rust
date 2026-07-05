//! Validation-error parity with the Python reference.
//!
//! `tests/fixtures/validation_parity.json` is generated from the upstream
//! `tests/fixtures/invalid_specs.py` battery: each entry is `{name, spec, types}`
//! where `types` is the exact set of error-type codes Python's `validate()`
//! emits. We feed the same spec to the Rust validator and assert it produces the
//! same set of codes — the equivalence bar Python itself uses (message-agnostic).

use std::collections::BTreeSet;

use anxwritter::input::{ChartData, Config};
use anxwritter::validation;

#[derive(serde::Deserialize)]
struct Case {
    name: String,
    spec: serde_json::Value,
    types: Vec<String>,
}

#[test]
fn validation_error_codes_match_python_reference() {
    let raw = include_str!("fixtures/validation_parity.json");
    let cases: Vec<Case> = serde_json::from_str(raw).expect("parse parity fixture");
    assert!(
        cases.len() >= 40,
        "expected the full battery, got {}",
        cases.len()
    );

    let mut failures = Vec::new();
    for case in &cases {
        let text = case.spec.to_string();
        // The whole spec feeds both Config and ChartData; lenient serde picks
        // the relevant keys (mirrors Python's `from_dict(whole_spec)`).
        let config: Config = serde_json::from_str(&text).unwrap_or_default();
        let data: ChartData = match serde_json::from_str(&text) {
            Ok(d) => d,
            Err(e) => {
                failures.push(format!(
                    "{}: spec failed to parse as ChartData: {e}",
                    case.name
                ));
                continue;
            }
        };
        let got: BTreeSet<String> = validation::validate(&config, &data)
            .iter()
            .map(|e| {
                serde_json::to_value(e.error_type)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        let want: BTreeSet<String> = case.types.iter().cloned().collect();
        if got != want {
            failures.push(format!(
                "{}: rust={:?} python={:?}",
                case.name,
                got.iter().collect::<Vec<_>>(),
                want.iter().collect::<Vec<_>>()
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "validation parity mismatches:\n  {}",
        failures.join("\n  ")
    );
}
