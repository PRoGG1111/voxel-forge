# Contributing

Start with a small problem you can reproduce. For geometry bugs, include the
voxel IDs, coordinates, neighbor sections, expected faces, and actual faces.
State whether a missing neighbor should be treated as air.

Run these checks from the repository root:

```console
cargo fmt --all -- --check
cargo clippy --offline --all-targets -- -D warnings
cargo test --offline
cargo test --offline --release
cargo doc --offline --no-deps
```

For storage changes, compare values against a dense reference and account for
bit-width boundaries, dead palette slots, and bulk replacement. For meshing
changes, preserve the independent unit-face oracle and winding checks.

Performance reports should include CPU, OS, compiler, build profile, fixture,
warmup, sampling method, and raw results. Separate heap usage from capacity and
meshing time from rendering time. A faster result on one machine is not a
general speed claim.

Keep the core free of renderer and game dependencies. Discuss API expansions
before implementing them. Contributions are provided under the project's MIT
license. Treat contributors respectfully and keep discussion specific to the
technical issue. No response-time commitment is established yet.
