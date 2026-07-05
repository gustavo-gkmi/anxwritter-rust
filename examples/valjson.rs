//! Emit validation-error signatures as JSON: `[{"type":..,"location":..}, ...]`.
//! The spec file is fed to BOTH Config and ChartData (lenient serde picks the
//! relevant keys), mirroring Python's `from_dict(whole_spec)`.
use anxwritter::input::{ChartData, Config};
use anxwritter::validation;
fn main() {
    let path = std::env::args().nth(1).expect("spec path");
    let text = std::fs::read_to_string(&path).unwrap();
    let config: Config = serde_json::from_str(&text).unwrap_or_default();
    let data: ChartData = match serde_json::from_str(&text) {
        Ok(d) => d,
        Err(e) => {
            println!("{{\"PARSE_ERROR\":\"{}\"}}", e);
            return;
        }
    };
    let errors = validation::validate(&config, &data);
    let sigs: Vec<_> = errors
        .iter()
        .map(|e| serde_json::json!({"type": e.error_type, "location": e.location}))
        .collect();
    println!("{}", serde_json::to_string(&sigs).unwrap());
}
