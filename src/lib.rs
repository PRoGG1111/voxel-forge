//! Compact, CPU-owned voxel sections and full-cube greedy meshing.
//!
//! All nonzero voxel IDs represent opaque unit cubes to the mesher. Storage
//! treats IDs as arbitrary `u32` values. No game registry or renderer is needed.
//!
//! ```
//! use voxel_forge::{GreedyMesher, Mesh, Neighborhood, SectionStorage, VoxelId};
//!
//! let section = SectionStorage::filled(VoxelId::new(1));
//! let mut mesh = Mesh::default();
//! GreedyMesher::default().mesh(&Neighborhood::new(&section), &mut mesh);
//! assert_eq!(mesh.quads().len(), 6);
//! assert_eq!(mesh.visible_unit_faces(), 6 * 16 * 16);
//! ```

mod coordinates;
mod meshing;
mod section;

// Compile the README's Rust example alongside the API example.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct ReadmeExamples;

pub use coordinates::{LocalPos, SECTION_EDGE, SECTION_VOLUME, SectionPos, WorldPos};
pub use meshing::{Face, GreedyMesher, Mesh, Neighborhood, Quad};
pub use section::{SectionMemory, SectionStorage, StorageKind, VoxelIter};

/// Application-defined material/state identifier. Zero is air.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct VoxelId(u32);

impl VoxelId {
    pub const AIR: Self = Self(0);
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
    pub const fn raw(self) -> u32 {
        self.0
    }
    pub const fn is_air(self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoxelError {
    InvalidLocal { x: u8, y: u8, z: u8 },
    InvalidIndex(usize),
    CoordinateOverflow,
    InvalidBlockCount(usize),
}

impl std::fmt::Display for VoxelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLocal { x, y, z } => {
                write!(f, "local coordinates ({x}, {y}, {z}) must be below 16")
            }
            Self::InvalidIndex(i) => write!(f, "local index {i} must be below {SECTION_VOLUME}"),
            Self::CoordinateOverflow => {
                f.write_str("section is outside the i64 world coordinate domain")
            }
            Self::InvalidBlockCount(n) => write!(f, "expected {SECTION_VOLUME} voxels, got {n}"),
        }
    }
}
impl std::error::Error for VoxelError {}
