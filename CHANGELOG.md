# Changelog

All notable changes to this crate are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this crate tracks a
specific upstream [`anxwritter`](https://github.com/gustavo-gkmi/anxwritter)
(Python) release — see `TARGET_ANXWRITTER_VERSION`.

## [1.24.0] - 2026-07-05

Initial public release. Full Rust port of the `anxwritter` Python library,
targeting upstream **1.24.2**.

> Version scheme: the crate's **MAJOR.MINOR** tracks the upstream `anxwritter`
> line (`1.24.x`); the **PATCH** is this port's own iteration counter and is
> deliberately independent of upstream's patch number. The exact upstream
> release targeted is always `anxwritter::TARGET_ANXWRITTER_VERSION` (here,
> `1.24.2`).

### Added

- Write i2 Analyst's Notebook Exchange (`.anx`) files (UTF-16 LE XML) from typed
  Rust data, JSON, or YAML.
- High-level API: `write_anx` (streaming), `build_anx` (materialized),
  `to_xml`/`render_xml`, and `iter_anx_bytes`.
- Ergonomic `prelude` with fluent constructors (`Icon::new`, `Link::new`,
  `ChartData::add_icon`/`add_link`, …).
- Validation with full error-code parity against the Python reference.
- Config layering (cascade merge/wipe/delete/lock) and the geometric layouts
  (grid/circle/radial/tree) match Python exactly; force-directed layouts
  (`fr`/`forceatlas2`) are algorithm-faithful and deterministic.
- CLI: `anxwritter [--config FILE]... [--validate-only] <input> [-o OUT.anx]`.

### Notes

- Output is content-identical to the Python reference (canonical/structural
  diff); the minimal no-config chart is byte-identical.
- Force-directed coordinates are not bit-identical to Python (floating-point
  summation order); the layout is faithful, deterministic, and valid.

[1.24.0]: https://github.com/gustavo-gkmi/anxwritter-rust/releases/tag/v1.24.0
