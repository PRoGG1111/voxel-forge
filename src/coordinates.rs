use super::VoxelError;

pub const SECTION_EDGE: usize = 16;
pub const SECTION_VOLUME: usize = SECTION_EDGE * SECTION_EDGE * SECTION_EDGE;

/// Integer block coordinates: +Y is up. Supports the full i64 domain.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorldPos {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}

impl WorldPos {
    pub const fn new(x: i64, y: i64, z: i64) -> Self {
        Self { x, y, z }
    }

    pub fn split(self) -> (SectionPos, LocalPos) {
        let section = SectionPos::new(
            self.x.div_euclid(16),
            self.y.div_euclid(16),
            self.z.div_euclid(16),
        );
        let local = LocalPos::pack(
            self.x.rem_euclid(16) as u16,
            self.y.rem_euclid(16) as u16,
            self.z.rem_euclid(16) as u16,
        );
        (section, local)
    }
}

/// Section coordinates, NOT block coordinates. Y addresses vertical sections.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SectionPos {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}

impl SectionPos {
    pub const fn new(x: i64, y: i64, z: i64) -> Self {
        Self { x, y, z }
    }

    pub fn world(self, local: LocalPos) -> Result<WorldPos, VoxelError> {
        // i128 also handles the last local block of the section containing MAX.
        fn axis(section: i64, local: u8) -> Result<i64, VoxelError> {
            i64::try_from(i128::from(section) * 16 + i128::from(local))
                .map_err(|_| VoxelError::CoordinateOverflow)
        }
        Ok(WorldPos::new(
            axis(self.x, local.x())?,
            axis(self.y, local.y())?,
            axis(self.z, local.z())?,
        ))
    }

    pub fn origin(self) -> Result<WorldPos, VoxelError> {
        self.world(LocalPos::ZERO)
    }

    /// Only complete sections inside the i64 block-coordinate domain are valid.
    pub fn validate(self) -> Result<(), VoxelError> {
        self.origin()?;
        self.world(LocalPos((SECTION_VOLUME - 1) as u16))?;
        Ok(())
    }

    pub fn offset(self, x: i64, y: i64, z: i64) -> Result<Self, VoxelError> {
        let result = Self::new(
            self.x
                .checked_add(x)
                .ok_or(VoxelError::CoordinateOverflow)?,
            self.y
                .checked_add(y)
                .ok_or(VoxelError::CoordinateOverflow)?,
            self.z
                .checked_add(z)
                .ok_or(VoxelError::CoordinateOverflow)?,
        );
        result.validate()?;
        Ok(result)
    }
}

/// Validated, packed local coordinates. Invalid indices cannot enter storage APIs.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalPos(u16);

impl LocalPos {
    pub const ZERO: Self = Self(0);

    pub fn new(x: u8, y: u8, z: u8) -> Result<Self, VoxelError> {
        if x >= 16 || y >= 16 || z >= 16 {
            return Err(VoxelError::InvalidLocal { x, y, z });
        }
        Ok(Self::pack(u16::from(x), u16::from(y), u16::from(z)))
    }

    const fn pack(x: u16, y: u16, z: u16) -> Self {
        Self((y << 8) | (z << 4) | x)
    }

    pub fn from_index(index: usize) -> Result<Self, VoxelError> {
        if index >= SECTION_VOLUME {
            return Err(VoxelError::InvalidIndex(index));
        }
        Ok(Self(index as u16))
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }

    pub const fn x(self) -> u8 {
        (self.0 & 15) as u8
    }

    pub const fn y(self) -> u8 {
        (self.0 >> 8) as u8
    }

    pub const fn z(self) -> u8 {
        ((self.0 >> 4) & 15) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_coordinates_use_floor_division() {
        for (world, section, local) in [
            (-33, -3, 15),
            (-32, -2, 0),
            (-17, -2, 15),
            (-16, -1, 0),
            (-15, -1, 1),
            (-1, -1, 15),
            (0, 0, 0),
            (15, 0, 15),
            (16, 1, 0),
            (17, 1, 1),
            (32, 2, 0),
        ] {
            let (s, l) = WorldPos::new(world, world, world).split();
            assert_eq!(s, SectionPos::new(section, section, section));
            assert_eq!(l, LocalPos::new(local, local, local).unwrap());
        }
    }

    #[test]
    fn round_trip_extremes_and_mixed_axes() -> Result<(), VoxelError> {
        for x in [i64::MIN, i64::MIN + 15, -17, -1, 0, 16, i64::MAX] {
            for y in [i64::MIN, -16, -1, 15, i64::MAX] {
                for z in [-33, 0, 31, i64::MAX] {
                    let world = WorldPos::new(x, y, z);
                    let (section, local) = world.split();
                    section.validate()?;
                    assert_eq!(section.world(local)?, world);
                }
            }
        }
        Ok(())
    }

    #[test]
    fn every_local_index_is_unique_and_x_contiguous() -> Result<(), VoxelError> {
        for index in 0..SECTION_VOLUME {
            let p = LocalPos::from_index(index)?;
            assert_eq!(LocalPos::new(p.x(), p.y(), p.z())?, p);
            assert_eq!(
                index,
                usize::from(p.y()) * 256 + usize::from(p.z()) * 16 + usize::from(p.x())
            );
        }
        assert_eq!(LocalPos::new(1, 0, 0)?.index(), 1);
        assert_eq!(LocalPos::new(0, 0, 1)?.index(), 16);
        assert_eq!(LocalPos::new(0, 1, 0)?.index(), 256);
        Ok(())
    }

    #[test]
    fn invalid_local_coordinates_and_indices_are_rejected() {
        for p in [(16, 0, 0), (0, 255, 0), (0, 0, 16)] {
            assert!(LocalPos::new(p.0, p.1, p.2).is_err());
        }
        assert_eq!(
            LocalPos::from_index(4096),
            Err(VoxelError::InvalidIndex(4096))
        );
        assert!(LocalPos::from_index(usize::MAX).is_err());
    }

    #[test]
    fn unrepresentable_sections_and_neighbors_are_rejected() {
        assert_eq!(
            SectionPos::new(i64::MAX, 0, 0).origin(),
            Err(VoxelError::CoordinateOverflow)
        );
        let (max, _) = WorldPos::new(i64::MAX, 0, 0).split();
        assert!(max.offset(1, 0, 0).is_err());
        let (min, _) = WorldPos::new(0, i64::MIN, 0).split();
        assert!(min.offset(0, -1, 0).is_err());
        assert_eq!(
            SectionPos::new(0, 0, 0).offset(-1, 1, -1).unwrap(),
            SectionPos::new(-1, 1, -1)
        );
    }
}
