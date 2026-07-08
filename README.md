# anxwritter (Rust)

[![crates.io](https://img.shields.io/crates/v/anxwritter.svg)](https://crates.io/crates/anxwritter)
[![docs.rs](https://img.shields.io/docsrs/anxwritter)](https://docs.rs/anxwritter)
[![license](https://img.shields.io/crates/l/anxwritter.svg)](LICENSE)

A Rust port of the [`anxwritter`](https://github.com/gustavo-gkmi/anxwritter)
Python library. It writes i2 Analyst's Notebook Exchange (`.anx`) chart files —
UTF-16 XML documents that i2 Analyst's Notebook 9+ opens directly — from typed
Rust data, JSON, or YAML.

Output is **content-identical** to the Python reference (validated against it
across an error-parity battery, a feature corpus, and the full example chart).

The one carve-out is the **force-directed and random layout modes** (`fr`,
`forceatlas2`, `random`): their coordinates are faithful, deterministic, and
valid, but *not* bit-identical to Python — a scalar implementation can't
reproduce numpy/BLAS summation order or CPython's Mersenne-Twister sequence. The
geometric layouts (`grid`/`circle`/`radial`/`tree`) match exactly.

## Why this exists

This port exists to run `.anx` generation as a **centralized HTTP service**: a
system sends chart data as JSON or YAML and gets a finished `.anx` back. That one
goal drives every design choice here — a dependency-free binary, bounded-memory
streaming, and parallel throughput.

**Why Rust, not Python, for this.** A service that turns requests into files
lives or dies on per-request cost and concurrency — exactly where the Python
library is constrained:

- **True parallelism.** No GIL, so worker threads build charts on every core at
  once; throughput scales with hardware instead of serializing on one
  interpreter lock.
- **Lower, bounded memory.** Streaming emits UTF-16 straight to the sink without
  materializing the whole document, so a large chart doesn't spike a request's
  footprint — more concurrent requests per machine.
- **Fast startup, tiny footprint.** A single static binary with no interpreter or
  dependency tree starts in milliseconds and ships in a minimal container — ideal
  for autoscaling and cold starts.
- **Predictable latency.** No GC pauses, so tail latencies stay tight under load.

**Why a shared service beats each app rolling its own.** The `.anx` format is
intricate (UTF-16 XML, minted ids, validation rules, config layering). Baking a
writer into every application means each one re-embeds that logic, tracks the
format, and drifts out of sync. Centralizing it gives you:

- **One source of truth** for the format contract and validation — every caller
  gets identical, correct output, and there is a single place to update when i2
  or the upstream library changes.
- **Language-agnostic callers.** Anything that can POST JSON/YAML can produce
  `.anx` — no need to run Python, embed this crate, or reimplement the format.
- **Centralized validation and error reporting**, plus one place to scale,
  monitor, and secure.

This is what `anxwritter` was designed for from the start: not merely a `.anx`
builder, but an **organization-level enforcer of charting standards** — config
layering, locked defaults, and validation exist precisely so that every chart an
organization produces is consistent and conformant. A centralized server is the
natural home for that role: the standards live in one deployment, and every team
and application inherits them automatically rather than each re-implementing (and
diverging from) the rules.

## Performance

Against the Python reference (streaming, the production default), on the same
machine: roughly **8–11× faster** per build, **~2–3× less memory**, and — because
there is no GIL — **dozens of times higher throughput under concurrency**
(near-linear thread scaling). Output is byte-equivalent. Benchmark with
`--release`.

## Documentation

This crate keeps its own docs intentionally thin and **defers to the Python
library's documentation** for chart concepts, the `.anx` format, and the full set
of config/style options: <https://github.com/gustavo-gkmi/anxwritter>.

API reference (types and functions) is on
[docs.rs](https://docs.rs/anxwritter).

## Install

```toml
[dependencies]
anxwritter = "1.24"
```

## Example

Build a chart with the fluent API, then stream it to a file:

```rust
use anxwritter::prelude::*;

let mut data = ChartData::default();
data.add_icon(
        Icon::new("alice", "Person")
            .label("Alice")
            .attr("Phone", "555-0100")
            .color("Red"),
    )
    .add_icon(Icon::new("bob", "Person"))
    .add_link(Link::new("alice", "bob").link_type("Call").directed());

let file = std::fs::File::create("chart.anx")?;
write_anx(&Config::default(), &data, file, true)?; // validate, then stream UTF-16
# Ok::<(), anxwritter::Error>(())
```

Or parse the same chart from JSON / YAML — both deserialize into the exact same
types:

```rust
use anxwritter::{ChartData, Config, build_anx};

let config = Config::from_json("{}")?;             // or Config::from_yaml(..)
let data = ChartData::from_json(
    r#"{"entities":{"icons":[{"id":"alice","type":"Person"}]},"links":[]}"#,
)?;
let bytes = build_anx(&config, &data, true)?;
# Ok::<(), anxwritter::Error>(())
```

Validation errors are returned as a structured list (see `validation::validate`).

## API

| Function | Validates | Output |
|---|---|---|
| `write_anx(cfg, data, w, validate)` | optional | **streams** UTF-16 `.anx` to a `Write` sink (bounded memory) |
| `build_anx(cfg, data, validate)` | optional | materialized `Vec<u8>` |
| `to_xml(cfg, data)` | yes | compact XML `String` (use `Builder::build_with(data, false)` for pretty/indented) |
| `render_xml(cfg, data)` | no | compact XML `String` (low-level) |
| `iter_anx_bytes(cfg, data, chunk)` | yes | chunked byte iterator |

## CLI

```sh
anxwritter [--config FILE]... [--validate-only] <input.json|yaml> [-o OUT.anx]
```

## Versioning

This crate **tracks a specific upstream `anxwritter` release** — exposed as
`anxwritter::TARGET_ANXWRITTER_VERSION` and embedded in each file's provenance
comment. The crate's `MAJOR.MINOR` mirrors the upstream line (`1.24.x`); the
**patch is this port's own iteration counter and is independent of upstream's
patch number** (e.g. crate `1.24.0` targets upstream `1.24.2`). Always read
`TARGET_ANXWRITTER_VERSION` for the exact upstream release. Breaking changes to
the Rust API bump the crate major so Cargo semver stays honest.

## Trademarks

Independent project — not affiliated with, endorsed by, or sponsored by IBM,
i2 Group, N.Harris Computer Corporation, or any other vendor. "i2", "i2 Analyst's
Notebook", and "ANB" are trademarks of their respective owners, referenced here
for nominative use only. See [NOTICE.md](NOTICE.md) for the full interoperability
and trademark statement.

## License

MIT — see [LICENSE](LICENSE). A Rust port of the MIT-licensed
[`anxwritter`](https://github.com/gustavo-gkmi/anxwritter) Python library by the
same author.

> Developed with the help of AI coding assistants.
