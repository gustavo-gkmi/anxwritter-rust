//! Concurrency throughput: build `total` charts across `threads` workers.
//! Usage: conc <config.json> <data.json> <total> <threads>
use anxwritter::builder::Builder;
use anxwritter::input::{ChartData, Config};
use std::sync::Arc;
use std::time::Instant;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let config: Arc<Config> =
        Arc::new(serde_json::from_str(&std::fs::read_to_string(&a[1]).unwrap()).unwrap());
    let data: Arc<ChartData> =
        Arc::new(serde_json::from_str(&std::fs::read_to_string(&a[2]).unwrap()).unwrap());
    let total: usize = a[3].parse().unwrap();
    let threads: usize = a[4].parse().unwrap();
    let t = Instant::now();
    let mut handles = Vec::new();
    for w in 0..threads {
        let (c, d) = (config.clone(), data.clone());
        let n = total / threads + if w < total % threads { 1 } else { 0 };
        handles.push(std::thread::spawn(move || {
            let mut sink = 0usize;
            for _ in 0..n {
                sink += Builder::new(&c).build(&d).len();
            }
            sink
        }));
    }
    let _: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
    println!("{:.0}", t.elapsed().as_secs_f64() * 1000.0);
}
