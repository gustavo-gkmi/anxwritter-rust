//! Build a chart purely from typed Rust — no JSON/YAML — using the fluent API.
//! Run: `cargo run --example native_demo`
use anxwritter::prelude::*;

fn main() {
    let mut data = ChartData::default();
    data.add_icon(
        Icon::new("alice", "Person")
            .label("Alice")
            .attr("Phone", "555-0100")
            .attr("Age", 30)
            .attr("Active", true)
            .color("Red")
            .card(Card::new("Sighting").date("2026-01-05").source("Witness")),
    )
    .add_icon(Icon::new("bob", "Person"))
    .add_link(
        Link::new("alice", "bob")
            .link_type("Call")
            .directed()
            .attr("weight", 5),
    );

    let xml = Builder::new(&Config::default()).build(&data);
    println!(
        "{} chars, link present: {}, card present: {}",
        xml.len(),
        xml.contains("Type=\"Call\""),
        xml.contains("<Card ")
    );
}
