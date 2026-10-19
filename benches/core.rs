//! Dependency-free microbenchmark. Samples time batches of repeated calls.
#[path = "../examples/common/mod.rs"]
mod common;

use std::{hint::black_box, time::Instant};
use voxel_forge::{GreedyMesher, Mesh, Neighborhood, SectionStorage};

fn main() {
    const SAMPLES: usize = 41;
    const CALLS: usize = 25;
    println!(
        "fixture,calls_per_sample,samples,mesh_p50_ns,mesh_p95_ns,mesh_p99_ns,unit_faces,quads,storage_inline_bytes,heap_used_bytes,heap_capacity_bytes,dense_bytes,mesh_used_bytes,mesh_capacity_bytes,scratch_bytes"
    );
    for name in ["empty", "solid", "terrain", "checkerboard", "unique"] {
        let blocks = common::fixture(name);
        let section = SectionStorage::from_voxels(&blocks).expect("valid fixture");
        let input = Neighborhood::new(&section);
        let mut mesher = GreedyMesher::default();
        let mut output = Mesh::default();
        // Warm scratch and output capacity before sampling.
        for _ in 0..10 {
            mesher.mesh(black_box(&input), black_box(&mut output));
        }
        let mut samples = [0_u128; SAMPLES];
        for sample in &mut samples {
            let start = Instant::now();
            for _ in 0..CALLS {
                mesher.mesh(black_box(&input), black_box(&mut output));
            }
            *sample = start.elapsed().as_nanos() / CALLS as u128;
            black_box(output.quads());
        }
        samples.sort_unstable();
        let percentile = |p: usize| samples[(SAMPLES * p).div_ceil(100) - 1];
        let mem = section.memory_usage();
        println!(
            "{name},{CALLS},{SAMPLES},{},{},{},{},{},{},{},{},{},{},{},{}",
            percentile(50),
            percentile(95),
            percentile(99),
            output.visible_unit_faces(),
            output.quads().len(),
            mem.inline_bytes,
            mem.heap_used_bytes,
            mem.heap_capacity_bytes,
            std::mem::size_of_val(&blocks),
            output.used_bytes(),
            output.capacity_bytes(),
            GreedyMesher::scratch_bytes()
        );
    }
}
