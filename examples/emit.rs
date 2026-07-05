//! Tiny end-to-end emitter: `emit <config.json> <data.json> <out.anx>`.
//!
//! Builds a chart from a single config layer + data file and writes a UTF-16
//! `.anx`. Used to diff Rust output against the Python reference; a real CLI
//! with config layering comes later.

use std::fs;

use anxwritter::builder::Builder;
use anxwritter::input::{ChartData, Config};
use anxwritter::xml::to_utf16le_with_bom;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: emit <config.json> <data.json> <out.anx> [pretty]");
        std::process::exit(2);
    }
    let config: Config = serde_json::from_str(&fs::read_to_string(&args[1]).unwrap()).unwrap();
    let data: ChartData = serde_json::from_str(&fs::read_to_string(&args[2]).unwrap()).unwrap();
    let pretty = args.get(4).map(|s| s == "pretty").unwrap_or(false);
    let xml = Builder::new(&config).build_with(&data, !pretty);
    fs::write(&args[3], to_utf16le_with_bom(&xml)).unwrap();
    eprintln!("wrote {}", args[3]);
}
