# Initial issue backlog

These are proposed issues, not evidence of existing external contributors.
Create them on GitHub after publishing the repository.

## 1. Reproduce the benchmark on Linux (good first issue)

Run the existing harness on a Linux machine and attach raw CSV plus environment
details. Acceptance: identical face/quad counts; any timing differences reported
with the same measurement method. No speed target is prescribed.

## 2. Add a minimal renderer integration example

Choose one renderer based on an actual contributor's use case. Acceptance:
render a 2×2 arrangement, edit a boundary voxel, and remesh both touching
sections without shared internal faces. Keep the renderer dependency outside
the core package. Report the renderer/toolchain versions actually tested.

## 3. Evaluate a dense fallback for high-diversity storage

The unique-ID fixture currently uses more heap than a dense section. Acceptance:
benchmark construction, random reads, edits, and memory at 2, 16, 256, and 4096
IDs; propose an explicit crossover rule before changing representation. Preserve
all existing round-trip and compaction invariants.

## 4. Document application-side boundary invalidation

Acceptance: a runnable example showing load, unload, and boundary edits in all
six directions, including negative world coordinates. Avoid an engine scheduler
in the core API.

## First month

Publish a usable pre-release and ask a few voxel-tool developers to try one
specific integration. Fix reproducible problems and record real feedback with
permission. Consider a registry release after the package name, public URL,
hosted checks, and API documentation are verified. External adoption is an
outcome to earn, not a release requirement to invent.
