//! Benchmark harness: parse + build timing and peak RSS for a config+data pair.
//! Usage: bench <config.json> <data.json> <iters>
use anxwritter::builder::Builder;
use anxwritter::input::{ChartData, Config};
use anxwritter::validation;
use std::time::Instant;

fn vmhwm_mb() -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM"))
                .map(|l| l.to_string())
        })
        .and_then(|l| {
            l.split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<f64>().ok())
        })
        .map(|kb| kb / 1024.0)
        .unwrap_or(0.0)
}
fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (cfg_t, data_t, iters) = (
        std::fs::read_to_string(&a[1]).unwrap(),
        std::fs::read_to_string(&a[2]).unwrap(),
        a[3].parse::<usize>().unwrap(),
    );
    let mut pt = Vec::new();
    for _ in 0..iters {
        let t = Instant::now();
        let _c: Config = serde_json::from_str(&cfg_t).unwrap();
        let _d: ChartData = serde_json::from_str(&data_t).unwrap();
        pt.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let config: Config = serde_json::from_str(&cfg_t).unwrap();
    let data: ChartData = serde_json::from_str(&data_t).unwrap();
    let mode = a.get(4).map(|s| s.as_str()).unwrap_or("build");
    let mut bt = Vec::new();
    let mut out = 0usize;
    for _ in 0..iters {
        let t = Instant::now();
        if mode == "full" {
            let _ = validation::validate(&config, &data);
        }
        let xml = Builder::new(&config).build(&data);
        bt.push(t.elapsed().as_secs_f64() * 1000.0);
        out = xml.len();
    }
    println!(
        "{{\"parse_ms\": {:.2}, \"build_ms\": {:.2}, \"rss_mb\": {:.1}, \"out_chars\": {}}}",
        median(pt),
        median(bt),
        vmhwm_mb(),
        out
    );
}
