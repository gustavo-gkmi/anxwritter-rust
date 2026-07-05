//! Property tests: determinism and input-form equivalence — the invariants a
//! server depends on (same input → same bytes; JSON and YAML are interchangeable).

use anxwritter::builder::Builder;
use anxwritter::input::{ChartData, Config};

#[derive(serde::Deserialize)]
struct Case {
    name: String,
    spec: serde_json::Value,
}

fn corpus() -> Vec<Case> {
    serde_json::from_str(include_str!("fixtures/output_corpus.json")).expect("corpus")
}

fn build(spec: &serde_json::Value) -> String {
    let text = spec.to_string();
    let config: Config = serde_json::from_str(&text).unwrap_or_default();
    let data: ChartData = serde_json::from_str(&text).unwrap();
    Builder::new(&config).build(&data)
}

/// Building the same chart twice yields byte-identical output (no map/iteration
/// nondeterminism) — across the whole corpus.
#[test]
fn build_is_deterministic() {
    for case in corpus() {
        let a = build(&case.spec);
        let b = build(&case.spec);
        assert_eq!(a, b, "non-deterministic output for spec '{}'", case.name);
    }
}

/// The same logical chart expressed as JSON and as YAML produces identical
/// output — the two input forms are fully interchangeable.
#[test]
fn json_and_yaml_inputs_are_equivalent() {
    for case in corpus() {
        let json_text = case.spec.to_string();
        // Round-trip the spec through YAML to get an equivalent YAML document.
        let yaml_text = serde_yaml_ng::to_string(&case.spec).expect("to yaml");

        let cfg_json: Config = serde_json::from_str(&json_text).unwrap_or_default();
        let data_json: ChartData = serde_json::from_str(&json_text).unwrap();
        let cfg_yaml: Config = serde_yaml_ng::from_str(&yaml_text).unwrap_or_default();
        let data_yaml: ChartData = serde_yaml_ng::from_str(&yaml_text).unwrap();

        let from_json = Builder::new(&cfg_json).build(&data_json);
        let from_yaml = Builder::new(&cfg_yaml).build(&data_yaml);
        assert_eq!(
            from_json, from_yaml,
            "JSON and YAML inputs diverged for spec '{}'",
            case.name
        );
    }
}
