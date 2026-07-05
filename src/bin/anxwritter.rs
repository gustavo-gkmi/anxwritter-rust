//! `anxwritter` CLI — convert JSON/YAML chart data to a `.anx` file.
//!
//! Usage:
//!   anxwritter [--config FILE]... [--validate-only] <input> [-o OUTPUT]
//!
//! Config layers are overlaid left-to-right, then the positional data file is
//! built. Format (JSON vs YAML) is detected by file extension. Mirrors the
//! Python CLI's common path; full config-cascade semantics come later.

use std::path::Path;
use std::process::ExitCode;

use anxwritter::api::write_anx;
use anxwritter::config_layering::ConfigStack;
use anxwritter::input::ChartData;
use anxwritter::validation;

/// Parse a config layer as a generic JSON value (so the layering engine can
/// deep-merge / wipe / delete / lock across multiple `--config` files).
fn parse_config_value(path: &str) -> Result<serde_json::Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    if is_yaml(path) {
        serde_yaml_ng::from_str(&text).map_err(|e| format!("{path}: {e}"))
    } else {
        serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
    }
}

fn parse_data(path: &str) -> Result<ChartData, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    if is_yaml(path) {
        ChartData::from_yaml(&text)
    } else {
        ChartData::from_json(&text)
    }
    .map_err(|e| format!("{path}: {e}"))
}

fn is_yaml(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".yaml") || lower.ends_with(".yml")
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut configs: Vec<String> = Vec::new();
    let mut input: Option<String> = None;
    let mut output: Option<String> = None;
    let mut validate_only = false;

    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--config" => configs.push(it.next().ok_or("--config needs a value")?.clone()),
            "-o" | "--output" => output = Some(it.next().ok_or("-o needs a value")?.clone()),
            "--validate-only" => validate_only = true,
            "-h" | "--help" => {
                eprintln!(
                    "usage: anxwritter [--config FILE]... [--validate-only] <input> [-o OUTPUT]"
                );
                return Ok(());
            }
            other if other.starts_with('-') => return Err(format!("unknown flag: {other}")),
            other => input = Some(other.to_string()),
        }
    }

    let input = input.ok_or("missing positional <input> data file")?;

    let mut stack = ConfigStack::new();
    for c in &configs {
        stack.apply(parse_config_value(c)?);
    }
    let (config, layer_conflicts) = stack.finish();
    for c in &layer_conflicts {
        eprintln!("  config-layer: [{}] {}", c.location, c.message);
    }
    let data = parse_data(&input)?;

    if validate_only {
        let errors = validation::validate(&config, &data);
        if errors.is_empty() {
            eprintln!("valid: no errors");
            return Ok(());
        }
        for e in &errors {
            eprintln!("  [{}] {}", e.location, e.message);
        }
        return Err(format!("{} validation error(s)", errors.len()));
    }

    let mut out_path = output.unwrap_or_else(|| {
        let stem = Path::new(&input)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("chart");
        format!("{stem}.anx")
    });
    if !out_path.to_lowercase().ends_with(".anx") {
        out_path.push_str(".anx");
    }

    // Stream straight to the file (bounded memory), validating first.
    let file = std::fs::File::create(&out_path).map_err(|e| format!("{out_path}: {e}"))?;
    let writer = std::io::BufWriter::new(file);
    write_anx(&config, &data, writer, true).map_err(|e| e.to_string())?;
    eprintln!("wrote {out_path}");
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
