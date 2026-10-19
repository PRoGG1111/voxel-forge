# Local microbenchmark

The checked-in CSV is an observed run on 2026-10-19, not a target or estimate.

- CPU: AMD Ryzen 7 5800H with Radeon Graphics, 8 cores / 16 logical processors.
- OS: Windows 11 Pro, 10.0.26200, x64.
- Compiler: rustc 1.99.0 (b940084d7 2026-09-28), LLVM 23.1.1.
- Target: x86_64-pc-windows-msvc. Cargo's default optimized bench profile.
- Command: `cargo bench --offline --bench core`.

Each fixture is one 16³ section with absent neighbors (air). Fixture definitions
are shared by the example and benchmark in `examples/common/mod.rs`. Terrain
is a deterministic sawtooth height field with two solid materials and air;
it is not a captured player world. The unique-ID case deliberately exposes
the memory cost of a high-diversity palette.

For each fixture the harness constructs storage outside timing, warms the
mesher/output ten times, then measures 41 batches of 25 calls each. It divides
each batch's elapsed nanoseconds by 25 and reports nearest-rank p50/p95/p99
of those **batch averages**. They are not individual-call tail latencies.
`black_box` protects inputs and outputs from being optimized away. Meshing
includes decoding, boundary preparation, visibility tests, and quad generation.
Output capacity is reused after warmup; cold allocation costs are excluded.

No CPU affinity, isolated core, frequency lock, or power-state control was
applied. Background activity and thermal state can affect these short runs.
No comparison with another implementation or GPU rendering was performed.
Construction/edit speed and loaded-neighbor overhead are not benchmarked here.

Memory columns account for inline storage and vector payload length/capacity.
They exclude allocator headers and process RSS. `dense_bytes` is the size of a
dense `[VoxelId; 4096]`, not a separately timed mesher. Mesh buffer bytes and
fixed mesher scratch are reported separately; inline `Mesh` size is excluded.

To preserve another run in PowerShell:

```powershell
cargo bench --offline --bench core | Set-Content -Encoding utf8 benchmarks/local.csv
```

Compare deterministic face/quad counts first, then repeated timing runs using
the same profile and sampling method. Keep original raw results and environment
details when publishing a performance claim.
