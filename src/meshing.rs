use crate::{LocalPos, SECTION_EDGE, SECTION_VOLUME, SectionStorage, VoxelId};

const AREA: usize = SECTION_EDGE * SECTION_EDGE;

/// Face order also indexes the six neighbor slots. +Y is up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Face {
    NegativeX,
    PositiveX,
    NegativeY,
    PositiveY,
    NegativeZ,
    PositiveZ,
}

impl Face {
    pub const ALL: [Self; 6] = [
        Self::NegativeX,
        Self::PositiveX,
        Self::NegativeY,
        Self::PositiveY,
        Self::NegativeZ,
        Self::PositiveZ,
    ];
    pub const fn normal(self) -> [i8; 3] {
        match self {
            Self::NegativeX => [-1, 0, 0],
            Self::PositiveX => [1, 0, 0],
            Self::NegativeY => [0, -1, 0],
            Self::PositiveY => [0, 1, 0],
            Self::NegativeZ => [0, 0, -1],
            Self::PositiveZ => [0, 0, 1],
        }
    }
    fn axes(self) -> (usize, usize, usize) {
        match self {
            Self::NegativeX | Self::PositiveX => (0, 1, 2),
            Self::NegativeY | Self::PositiveY => (1, 2, 0),
            Self::NegativeZ | Self::PositiveZ => (2, 0, 1),
        }
    }
    fn positive(self) -> bool {
        self as u8 & 1 != 0
    }
}

/// Section-local rectangle. Extent axes are Y/Z for X faces, Z/X for Y,
/// and X/Y for Z. Coordinates run from 0 to 16, including the outer plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quad {
    pub origin: [u8; 3],
    pub extent: [u8; 2],
    pub face: Face,
    pub material: VoxelId,
}

impl Quad {
    /// Counterclockwise vertices viewed from outside; triangles (0,1,2),(0,2,3).
    pub fn vertices(self) -> [[u8; 3]; 4] {
        let (_, u, v) = self.face.axes();
        let a = self.origin;
        let mut b = a;
        b[u] += self.extent[0];
        let mut c = b;
        c[v] += self.extent[1];
        let mut d = a;
        d[v] += self.extent[1];
        if self.face.positive() {
            [a, b, c, d]
        } else {
            [a, d, c, b]
        }
    }
    pub fn unit_faces(self) -> usize {
        usize::from(self.extent[0]) * usize::from(self.extent[1])
    }
}

/// Missing neighbors are treated as air. Remesh a section when a touching
/// neighbor is loaded, unloaded or edited; no cache or invalidation is implicit.
pub struct Neighborhood<'a> {
    pub center: &'a SectionStorage,
    pub neighbors: [Option<&'a SectionStorage>; 6],
}

impl<'a> Neighborhood<'a> {
    pub fn new(center: &'a SectionStorage) -> Self {
        Self {
            center,
            neighbors: [None; 6],
        }
    }
    pub fn with_neighbor(mut self, face: Face, section: &'a SectionStorage) -> Self {
        self.neighbors[face as usize] = Some(section);
        self
    }
}

/// Reusable output; repeated meshing clears length and retains capacity.
#[derive(Debug, Default, Clone)]
pub struct Mesh {
    quads: Vec<Quad>,
    visible_unit_faces: usize,
}

impl Mesh {
    pub fn quads(&self) -> &[Quad] {
        &self.quads
    }
    /// Culled unit faces before greedy merging, rather than six per solid voxel.
    pub fn visible_unit_faces(&self) -> usize {
        self.visible_unit_faces
    }
    pub fn used_bytes(&self) -> usize {
        self.quads.len() * std::mem::size_of::<Quad>()
    }
    /// Quad buffer only; excludes this struct and allocator overhead.
    pub fn capacity_bytes(&self) -> usize {
        self.quads.capacity() * std::mem::size_of::<Quad>()
    }
}

/// Fixed reusable scratch. Decode once, then greedily merge equal material IDs
/// in each oriented slice. All nonzero IDs occlude each other.
pub struct GreedyMesher {
    blocks: [VoxelId; SECTION_VOLUME],
    borders: [[VoxelId; AREA]; 6],
    mask: [VoxelId; AREA],
}

impl Default for GreedyMesher {
    fn default() -> Self {
        Self {
            blocks: [VoxelId::AIR; SECTION_VOLUME],
            borders: [[VoxelId::AIR; AREA]; 6],
            mask: [VoxelId::AIR; AREA],
        }
    }
}

fn index(p: [usize; 3]) -> usize {
    p[1] * AREA + p[2] * SECTION_EDGE + p[0]
}

impl GreedyMesher {
    pub const fn scratch_bytes() -> usize {
        std::mem::size_of::<Self>()
    }

    /// Replaces output. Only face neighbors affect cube visibility. This is
    /// deterministic, with no GPU, per-voxel heap allocation or unsafe code.
    pub fn mesh(&mut self, input: &Neighborhood<'_>, output: &mut Mesh) {
        output.quads.clear();
        output.visible_unit_faces = 0;
        input.center.copy_voxels(&mut self.blocks);
        for face in Face::ALL {
            let (axis, u, v) = face.axes();
            let border = &mut self.borders[face as usize];
            border.fill(VoxelId::AIR);
            if let Some(neighbor) = input.neighbors[face as usize] {
                for j in 0..SECTION_EDGE {
                    for i in 0..SECTION_EDGE {
                        let mut p = [0; 3];
                        p[axis] = if face.positive() { 0 } else { 15 };
                        p[u] = i;
                        p[v] = j;
                        border[j * SECTION_EDGE + i] = neighbor
                            .get(LocalPos::from_index(index(p)).expect("valid local index"));
                    }
                }
            }
            for layer in 0..SECTION_EDGE {
                self.mask.fill(VoxelId::AIR);
                for j in 0..SECTION_EDGE {
                    for i in 0..SECTION_EDGE {
                        let mut p = [0; 3];
                        p[axis] = layer;
                        p[u] = i;
                        p[v] = j;
                        let material = self.blocks[index(p)];
                        if material.is_air() {
                            continue;
                        }
                        let adjacent = if (face.positive() && layer == 15)
                            || (!face.positive() && layer == 0)
                        {
                            self.borders[face as usize][j * SECTION_EDGE + i]
                        } else {
                            p[axis] = if face.positive() {
                                layer + 1
                            } else {
                                layer - 1
                            };
                            self.blocks[index(p)]
                        };
                        if adjacent.is_air() {
                            self.mask[j * SECTION_EDGE + i] = material;
                            output.visible_unit_faces += 1;
                        }
                    }
                }
                self.merge_slice(face, layer, output);
            }
        }
    }

    fn merge_slice(&mut self, face: Face, layer: usize, output: &mut Mesh) {
        let (axis, u, v) = face.axes();
        for j in 0..SECTION_EDGE {
            let mut i = 0;
            while i < SECTION_EDGE {
                let material = self.mask[j * SECTION_EDGE + i];
                if material.is_air() {
                    i += 1;
                    continue;
                }
                let mut width = 1;
                while i + width < SECTION_EDGE
                    && self.mask[j * SECTION_EDGE + i + width] == material
                {
                    width += 1;
                }
                let mut height = 1;
                while j + height < SECTION_EDGE
                    && (0..width)
                        .all(|dx| self.mask[(j + height) * SECTION_EDGE + i + dx] == material)
                {
                    height += 1;
                }
                for dy in 0..height {
                    self.mask[(j + dy) * SECTION_EDGE + i..(j + dy) * SECTION_EDGE + i + width]
                        .fill(VoxelId::AIR);
                }
                let mut origin = [0; 3];
                origin[axis] = (layer + usize::from(face.positive())) as u8;
                origin[u] = i as u8;
                origin[v] = j as u8;
                output.quads.push(Quad {
                    origin,
                    extent: [width as u8, height as u8],
                    face,
                    material,
                });
                i += width;
            }
        }
    }
}
