//! Export a synthetic two-material terrain section as a standard OBJ file.
mod common;

use std::{
    fs::File,
    io::{BufWriter, Write},
};
use voxel_forge::{GreedyMesher, Mesh, Neighborhood, SectionStorage};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "terrain.obj".into());
    let section = SectionStorage::from_voxels(&common::fixture("terrain"))?;
    let mut mesh = Mesh::default();
    GreedyMesher::default().mesh(&Neighborhood::new(&section), &mut mesh);
    let mut file = BufWriter::new(File::create(&path)?);
    writeln!(
        file,
        "# voxel-forge synthetic terrain; +Y up; units = voxels"
    )?;
    for quad in mesh.quads() {
        for [x, y, z] in quad.vertices() {
            writeln!(file, "v {x} {y} {z}")?;
        }
    }
    for (i, quad) in mesh.quads().iter().enumerate() {
        writeln!(file, "g material_{}", quad.material.raw())?;
        let a = i * 4 + 1;
        writeln!(file, "f {a} {} {}", a + 1, a + 2)?;
        writeln!(file, "f {a} {} {}", a + 2, a + 3)?;
    }
    file.flush()?;
    let memory = section.memory_usage();
    println!(
        "{path}: {} visible unit faces -> {} quads ({} triangles)",
        mesh.visible_unit_faces(),
        mesh.quads().len(),
        mesh.quads().len() * 2
    );
    println!(
        "Storage: {} inline + {} used heap bytes; {} reserved heap bytes",
        memory.inline_bytes, memory.heap_used_bytes, memory.heap_capacity_bytes
    );
    Ok(())
}
