# M0 baseline

Measured on 2026-11-14, macOS arm64, Rust `1.91.1`, debug binary, bundled SQLite.

The benchmark creates a fresh store, ingests generated events, searches the
same scope, and runs consolidation once. Each command runs once, so these
numbers are directional and are not production SLAs.

| Events | Ingest | Search | Consolidate | SQLite |
|---:|---:|---:|---:|---:|
| 1,000 | 0.055 s | 0.007 s | 0.162 s | 200,704 bytes |
| 10,000 | 0.387 s | 0.020 s | 0.596 s | 1,245,184 bytes |

Dataset: generated events with stable IDs, scope `bench/project`, and text
`ordinary event <n>`. Search query: `ordinary event`, limit default. No durable
memory candidates were extracted from the dataset.

Verification:

- `cargo test`: passed.
- `cargo fmt --check`: blocked because `rustfmt` is not installed.
- `cargo clippy --all-targets -- -D warnings`: blocked because `clippy` is not installed.
