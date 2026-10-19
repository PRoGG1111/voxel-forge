use std::collections::HashSet;
use voxel_forge::{
    Face, GreedyMesher, LocalPos, Mesh, Neighborhood, SECTION_VOLUME, SectionStorage, VoxelId,
};

type UnitFace = ([u8; 3], u8, u32);

fn voxel(storage: &SectionStorage, p: [u8; 3]) -> VoxelId {
    storage.get(LocalPos::new(p[0], p[1], p[2]).unwrap())
}

// Independently enumerate visible unit faces from voxel/neighbor lookups.
fn oracle(input: &Neighborhood<'_>) -> HashSet<UnitFace> {
    let mut expected = HashSet::new();
    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                let p = [x, y, z];
                let id = voxel(input.center, p);
                if id.is_air() {
                    continue;
                }
                for face in Face::ALL {
                    let n = face.normal();
                    let shifted =
                        std::array::from_fn::<_, 3, _>(|a| i16::from(p[a]) + i16::from(n[a]));
                    let inside = shifted.iter().all(|&v| (0..16).contains(&v));
                    let adjacent = if inside {
                        voxel(input.center, shifted.map(|v| v as u8))
                    } else if let Some(neighbor) = input.neighbors[face as usize] {
                        voxel(neighbor, shifted.map(|v| v.rem_euclid(16) as u8))
                    } else {
                        VoxelId::AIR
                    };
                    if adjacent.is_air() {
                        expected.insert((p, face as u8, id.raw()));
                    }
                }
            }
        }
    }
    expected
}

fn verify(input: &Neighborhood<'_>, mesh: &Mesh) {
    let expected = oracle(input);
    let mut actual = HashSet::new();
    for quad in mesh.quads() {
        let n = quad.face.normal();
        let axis = n.iter().position(|&v| v != 0).unwrap();
        let (u, v) = match axis {
            0 => (1, 2),
            1 => (2, 0),
            _ => (0, 1),
        };
        assert!(quad.extent.iter().all(|&v| (1..=16).contains(&v)));
        for vertex in quad.vertices() {
            assert!(vertex.iter().all(|&v| v <= 16));
        }
        let verts = quad.vertices().map(|p| p.map(i32::from));
        let a: [i32; 3] = std::array::from_fn(|i| verts[1][i] - verts[0][i]);
        let b: [i32; 3] = std::array::from_fn(|i| verts[2][i] - verts[0][i]);
        let cross = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        assert!(
            cross
                .iter()
                .zip(n)
                .map(|(c, n)| c * i32::from(n))
                .sum::<i32>()
                > 0
        );
        for dv in 0..quad.extent[1] {
            for du in 0..quad.extent[0] {
                let mut p = quad.origin;
                if n[axis] > 0 {
                    p[axis] -= 1;
                }
                p[u] += du;
                p[v] += dv;
                assert!(
                    actual.insert((p, quad.face as u8, quad.material.raw())),
                    "overlapping quads"
                );
            }
        }
    }
    assert_eq!(mesh.visible_unit_faces(), expected.len());
    assert_eq!(actual, expected);
}

#[test]
fn empty_solid_and_all_six_shared_borders() {
    let solid = SectionStorage::filled(VoxelId::new(7));
    let mut mesher = GreedyMesher::default();
    let mut mesh = Mesh::default();
    let mut input = Neighborhood::new(&solid);
    mesher.mesh(&input, &mut mesh);
    assert_eq!(mesh.quads().len(), 6);
    assert_eq!(mesh.visible_unit_faces(), 1536);
    verify(&input, &mesh);
    for face in Face::ALL {
        input = input.with_neighbor(face, &solid);
        mesher.mesh(&input, &mut mesh);
        assert_eq!(mesh.quads().len(), 5 - face as usize);
        verify(&input, &mesh);
    }
    let empty = SectionStorage::default();
    let empty_input = Neighborhood::new(&empty);
    mesher.mesh(&empty_input, &mut mesh);
    verify(&empty_input, &mesh);
}

#[test]
fn checkerboard_has_no_mergeable_faces() {
    let blocks = std::array::from_fn::<_, SECTION_VOLUME, _>(|i| {
        VoxelId::new(((i % 16 + i / 16 % 16 + i / 256) % 2) as u32)
    });
    let section = SectionStorage::from_voxels(&blocks).unwrap();
    let input = Neighborhood::new(&section);
    let mut mesh = Mesh::default();
    GreedyMesher::default().mesh(&input, &mut mesh);
    assert_eq!(mesh.quads().len(), 12288);
    verify(&input, &mesh);
}

#[test]
fn neighboring_materials_occlude_but_never_merge_different_ids() {
    let blocks = std::array::from_fn::<_, SECTION_VOLUME, _>(|i| {
        VoxelId::new(if i % 16 < 8 { 100 } else { u32::MAX })
    });
    let section = SectionStorage::from_voxels(&blocks).unwrap();
    let input = Neighborhood::new(&section);
    let mut mesh = Mesh::default();
    GreedyMesher::default().mesh(&input, &mut mesh);
    assert_eq!(mesh.quads().len(), 10);
    verify(&input, &mesh);
}

#[test]
fn randomized_neighbors_match_oracle_and_reused_buffers_stay_clean() {
    let mut seed = 0x7964_c0de_u32;
    let mut mesher = GreedyMesher::default();
    let mut mesh = Mesh::default();
    for round in 0..12 {
        let sections: [SectionStorage; 7] = std::array::from_fn(|_| {
            let blocks = std::array::from_fn::<_, SECTION_VOLUME, _>(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let id = if round % 2 == 0 {
                    seed % 5
                } else {
                    u32::from(seed % 4 == 0)
                };
                VoxelId::new(id)
            });
            SectionStorage::from_voxels(&blocks).unwrap()
        });
        let mut input = Neighborhood::new(&sections[0]);
        for face in Face::ALL {
            input.neighbors[face as usize] = Some(&sections[face as usize + 1]);
        }
        mesher.mesh(&input, &mut mesh);
        verify(&input, &mesh);
        let previous = mesh.quads().to_vec();
        mesher.mesh(&input, &mut mesh);
        assert_eq!(mesh.quads(), previous);
        let empty = SectionStorage::default();
        mesher.mesh(&Neighborhood::new(&empty), &mut mesh);
        assert!(mesh.quads().is_empty());
    }
}
