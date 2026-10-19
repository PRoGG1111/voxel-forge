use voxel_forge::{SECTION_VOLUME, VoxelId};

pub fn fixture(name: &str) -> [VoxelId; SECTION_VOLUME] {
    std::array::from_fn(|i| {
        let x = i % 16;
        let z = i / 16 % 16;
        let y = i / 256;
        match name {
            "empty" => VoxelId::AIR,
            "solid" => VoxelId::new(1),
            "checkerboard" => VoxelId::new(((x + y + z) % 2) as u32),
            "unique" => VoxelId::new(i as u32 + 1),
            "terrain" => {
                let height = 4 + ((x * 3 + z * 5) % 9);
                if y > height {
                    VoxelId::AIR
                } else if y == height {
                    VoxelId::new(2)
                } else {
                    VoxelId::new(1)
                }
            }
            _ => panic!("unknown fixture: {name}"),
        }
    })
}
