//! Peak-RSS probe isolating the output-buffer cost: parse once, then either
//! materialize the full .anx bytes or stream them to a sink.
//! Usage: memprobe <config.json> <data.json> <materialize|stream>
use anxwritter::api::{build_anx, write_anx};
use anxwritter::input::{ChartData, Config};
fn vmhwm_mb() -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM"))
                .map(str::to_string)
        })
        .and_then(|l| {
            l.split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<f64>().ok())
        })
        .map(|kb| kb / 1024.0)
        .unwrap_or(0.0)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let config: Config = serde_json::from_str(&std::fs::read_to_string(&a[1]).unwrap()).unwrap();
    let data: ChartData = serde_json::from_str(&std::fs::read_to_string(&a[2]).unwrap()).unwrap();
    let before = vmhwm_mb();
    if a[3] == "materialize" {
        let bytes = build_anx(&config, &data, false).unwrap();
        std::hint::black_box(bytes.len());
        println!(
            "materialize: peakRSS={:.1} MB (output Vec held)",
            vmhwm_mb()
        );
    } else {
        write_anx(&config, &data, std::io::sink(), false).unwrap();
        println!("stream:      peakRSS={:.1} MB (64 KiB buffer)", vmhwm_mb());
    }
    eprintln!("(rss before output step: {:.1} MB)", before);
}
