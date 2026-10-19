# Provenance

Prepared on 2026-10-19 from the owner's local Rust/Vulkan voxel engine.

`src/section.rs` adapts the engine's `src/world/section.rs`. The palette
representation, bit packing, edit rules, and applicable tests are retained.
Game block registries and block-shape wrappers were replaced with an opaque
`VoxelId(u32)` API. `src/coordinates.rs` adapts
`src/world/coordinates.rs`, changing the error type to the standalone one.

For reproducibility, the original files at extraction had these SHA-256 hashes:

| Original file | SHA-256 |
| --- | --- |
| `src/world/section.rs` | `1e3c704af8e5dbf98b7976f2850ab04dec109e7463050e929c69f25973b749ab` |
| `src/world/coordinates.rs` | `76e7e0a33ce8a2f40b1ffc18efabbe7c155b1d0fbd0c27a330b5892020a154a4` |

The source checkout had no committed history at extraction, so no upstream
commit ID is claimed. The hashes identify local inputs, not public releases.

`src/meshing.rs`, the standalone error/ID API, independent meshing tests,
fixtures, OBJ example, and microbenchmark were prepared for this package.
This is a simpler full-cube algorithm; the engine's game-specific renderer,
shaders, textures, lighting, and gameplay code are not bundled.

Preparation was assisted by an AI coding agent. Tests and benchmark results
describe local checks, not independent review or external adoption.
