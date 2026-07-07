# Changelog

All notable changes to this crate are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this crate tracks a
specific upstream [`anxwritter`](https://github.com/gustavo-gkmi/anxwritter)
(Python) release — see `TARGET_ANXWRITTER_VERSION`.

## [1.24.1] - 2026-07-07

Additive public-API changes so a downstream crate (an HTTP service wrapping this
library) can drive layered config application, error provenance, and vocabulary
discovery **without reaching into private items**. Output is unchanged — the
byte-equivalence battery is untouched and still passes.

### Added

- **Per-call cascade mode override.** [`ConfigStack::apply_with(layer, mode,
  source)`] applies a layer while optionally overriding its embedded
  `cascade.mode`. The new [`CascadeMode`] enum (`Merge`/`Wipe`/`Delete`/`Lock`)
  maps one-to-one to the Python `apply_config(operation=, wipe_previous=, lock=)`
  kwargs. `mode = None` preserves the previous behaviour (honour the embedded
  `cascade`, else default merge); `ConfigStack::apply(layer)` is now exactly
  `apply_with(layer, None, None)`.
- **Layer provenance on conflicts.** `apply_with`'s `source` argument tags any
  `locked_override` / `delete_contract` [`ValidationError`] this layer produces:
  the triggering layer's label on `source`, and — for `locked_override` — the
  label of the layer that *established* the lock on `config_source`. (The
  `ValidationError` fields `source` / `config_source` were already public.)
- **Discovery module** ([`discovery`]) for generating a service `/meta`
  endpoint from the crate itself: `discovery::enums()` (every public enum with
  its serialized values), `discovery::named_colors()`, and
  `discovery::arrange_algorithms()` / `discovery::arrange_aliases()`.
- **Enum introspection.** The new [`EnumMeta`] trait exposes `VARIANTS`,
  `as_str()`, and `values()` for the public config/style enums; a test pins the
  strings to the serde serialization so they cannot drift from the wire format.
- **Arrange tables.** `layout::ARRANGE_ALIASES` and `layout::ARRANGE_ALGORITHMS`
  are now public consts (the single source of truth `normalize_arrange` reads).
- Re-exports at the crate root: `CascadeMode`, `ConfigStack`, `EnumMeta`.

### Notes

- `layout::place_random` intentionally diverges from Python's MT19937 seeding;
  its coordinates are valid, deterministic, and stable but not byte-equivalent.
  Downstream parity batteries should exclude the `random` arrange mode (all other
  geometric modes — grid/circle/radial/tree — match upstream exactly).

[1.24.1]: https://github.com/gustavo-gkmi/anxwritter-rust/releases/tag/v1.24.1

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
