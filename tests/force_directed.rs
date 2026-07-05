//! Force-directed layout (fr / forceatlas2) invariants.
//!
//! These use the same published force laws as the Python reference (repulsion
//! `k²/d`, attraction `d²/k`, linear cooling, 0.5-pixel early stop) and are fully
//! deterministic. Exact coordinate parity with Python is *not* asserted: Python
//! computes the forces with numpy/BLAS matrix products whose floating-point
//! summation order is library- and hardware-specific and cannot be reproduced
//! bit-for-bit by a scalar implementation. Per the project's "valid is enough"
//! bar, we verify the layout is deterministic, well-formed, and places every
//! entity — not that the integer coordinates match numpy exactly.

use anxwritter::builder::Builder;
use anxwritter::input::{ChartData, Config};

fn chart(arrange: &str, n: usize) -> (Config, ChartData) {
    let icons: Vec<String> = (0..n)
        .map(|i| format!(r#"{{"id":"e{i}","type":"P"}}"#))
        .collect();
    let links: Vec<String> = (1..n)
        .map(|i| format!(r#"{{"from_id":"e0","to_id":"e{i}","type":"L"}}"#))
        .collect();
    let cfg: Config = serde_json::from_str(&format!(
        r#"{{"settings":{{"extra_cfg":{{"arrange":"{arrange}"}}}}}}"#
    ))
    .unwrap();
    let data: ChartData = serde_json::from_str(&format!(
        r#"{{"entities":{{"icons":[{}]}},"links":[{}]}}"#,
        icons.join(","),
        links.join(",")
    ))
    .unwrap();
    (cfg, data)
}

#[test]
fn force_directed_is_deterministic() {
    for arrange in ["fr", "forceatlas2"] {
        let (cfg, data) = chart(arrange, 12);
        let a = Builder::new(&cfg).build(&data);
        let b = Builder::new(&cfg).build(&data);
        assert_eq!(a, b, "{arrange} layout is non-deterministic");
    }
}

#[test]
fn force_directed_places_every_entity_and_is_well_formed() {
    for arrange in ["fr", "forceatlas2"] {
        let (cfg, data) = chart(arrange, 12);
        let xml = Builder::new(&cfg).build(&data);
        // 12 entities + 11 links = 23 chart items, each with an <End>/<Link>.
        assert_eq!(
            xml.matches("<ChartItem ").count(),
            23,
            "{arrange}: chart item count"
        );
        assert_eq!(
            xml.matches("<Entity ").count(),
            12,
            "{arrange}: entity count"
        );
        // Well-formed: balanced Chart and a non-empty spread of positions.
        assert!(xml.contains("<Chart>") && xml.trim_end().ends_with("</Chart>"));
        let xs: std::collections::HashSet<&str> = xml
            .lines()
            .filter_map(|l| l.split("XPosition=\"").nth(1))
            .filter_map(|s| s.split('"').next())
            .collect();
        assert!(
            xs.len() > 1,
            "{arrange}: entities should spread out, got {xs:?}"
        );
    }
}
