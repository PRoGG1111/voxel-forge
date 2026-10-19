# Local validation — 2026-10-19

Environment: Rust 1.99.0, x86_64-pc-windows-msvc, Windows 11 Pro.

| Check | Observed result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --offline --all-targets -- -D warnings` | Pass, no warnings |
| Debug unit/integration tests | 18 unit + 4 meshing tests pass |
| `cargo test --offline --doc` | 2 pass, including README example |
| `cargo test --offline --release` | All 24 unit/integration/doc tests pass |
| `RUSTDOCFLAGS="-D warnings" cargo doc --offline --no-deps` | Pass |
| `cargo run --offline --release --example terrain -- terrain.obj` | 1,334 quads / 2,668 triangles exported |
| `cargo bench --offline --bench core` | Five fixtures; raw CSV checked in |
| `cargo package --allow-dirty --offline` | Package generated and compiled successfully |

The documentation command's environment-variable notation above is shell
neutral; PowerShell sets it as `$env:RUSTDOCFLAGS = '-D warnings'` first.
Cargo reports a metadata warning because the public repository, homepage, and
documentation URL do not exist yet. No placeholder public URL was invented.

The reference tests compare expanded mesh rectangles against independent unit
face enumeration, including six randomized neighbors, material boundaries,
winding, repeated-buffer reuse, and a non-mergeable checkerboard. Storage tests
cover all palette widths, cross-word packing, a 4096-entry palette, deterministic
random edits, compaction, invalid lengths, and buffer reuse. Coordinate tests
cover negative values and i64 limits.

The OBJ was also rendered with the optional Pillow helper; the checked-in PNG
was visually inspected. This verifies an example's geometry preview, not a
renderer integration.

Hosted CI, Linux, the declared Rust 1.85 minimum, third-party renderers, and
crates.io publication have not been tested in this session. No independent
review, production usage, or external user count is implied by these checks.
