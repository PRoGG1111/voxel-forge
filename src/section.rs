use std::{iter::FusedIterator, mem::size_of};

use super::{LocalPos, SECTION_VOLUME, VoxelError, VoxelId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageKind {
    Empty,
    Uniform,
    Palette,
}

/// Owned payload accounting. Capacities include reserved but unused bytes.
/// Allocator headers/alignment overhead and process RSS are deliberately excluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionMemory {
    pub inline_bytes: usize,
    pub heap_used_bytes: usize,
    pub heap_capacity_bytes: usize,
    pub palette_entries: usize,
    pub bits_per_index: u8,
}

impl SectionMemory {
    pub fn owned_bytes(self) -> usize {
        self.inline_bytes + self.heap_capacity_bytes
    }
}

#[derive(Debug, Clone, Copy)]
struct PaletteEntry {
    id: VoxelId,
    count: u16, // 4096 fits: enables uniform collapse without scanning all blocks.
}

#[derive(Debug, Clone)]
struct Palette {
    entries: Vec<PaletteEntry>,
    words: Vec<u64>,
    bits: u8,
}

#[derive(Debug, Clone)]
enum Representation {
    Empty,
    Uniform(VoxelId),
    Palette(Palette),
}

/// 4096 values, no allocation for air or a uniform section. Mixed data uses
/// 1..12-bit palette indices packed continuously across u64 word boundaries.
/// Counts permit reuse of dead palette slots and O(1) collapse to uniform.
#[derive(Debug, Clone)]
pub struct SectionStorage {
    data: Representation,
}

impl Default for SectionStorage {
    fn default() -> Self {
        Self::filled(VoxelId::AIR)
    }
}

impl SectionStorage {
    pub fn filled(id: VoxelId) -> Self {
        Self {
            data: if id == VoxelId::AIR {
                Representation::Empty
            } else {
                Representation::Uniform(id)
            },
        }
    }

    /// Bulk construction chooses the final width before allocating packed words.
    pub fn from_voxels(blocks: &[VoxelId]) -> Result<Self, VoxelError> {
        if blocks.len() != SECTION_VOLUME {
            return Err(VoxelError::InvalidBlockCount(blocks.len()));
        }
        let first = blocks[0];
        if blocks.iter().all(|&id| id == first) {
            return Ok(Self::filled(first));
        }
        let mut entries: Vec<PaletteEntry> = Vec::with_capacity(4);
        // The temporary indices are stack-owned, not one allocation per block.
        let mut indices = [0_u16; SECTION_VOLUME];
        for (index, &id) in blocks.iter().enumerate() {
            let slot = match entries.iter().position(|entry| entry.id == id) {
                Some(slot) => slot,
                None => {
                    entries.push(PaletteEntry { id, count: 0 });
                    entries.len() - 1
                }
            };
            entries[slot].count += 1;
            indices[index] = slot as u16;
        }
        entries.shrink_to_fit();
        let bits = index_bits(entries.len());
        let mut words = vec![0; word_count(bits)];
        for (index, &slot) in indices.iter().enumerate() {
            write_index(&mut words, bits, index, usize::from(slot));
        }
        Ok(Self {
            data: Representation::Palette(Palette {
                entries,
                words,
                bits,
            }),
        })
    }

    pub fn kind(&self) -> StorageKind {
        match self.data {
            Representation::Empty => StorageKind::Empty,
            Representation::Uniform(_) => StorageKind::Uniform,
            Representation::Palette(_) => StorageKind::Palette,
        }
    }

    /// Replace bulk data while retaining mixed-output palette/word capacities.
    /// Uniform output still releases heap storage. Invalid lengths leave data
    /// untouched. Scratch indices are stack-owned, not a fresh heap allocation.
    pub fn replace_from_voxels(&mut self, blocks: &[VoxelId]) -> Result<(), VoxelError> {
        if blocks.len() != SECTION_VOLUME {
            return Err(VoxelError::InvalidBlockCount(blocks.len()));
        }
        let first = blocks[0];
        if blocks.iter().all(|&id| id == first) {
            self.fill(first);
            return Ok(());
        }
        if !matches!(self.data, Representation::Palette(_)) {
            self.data = Representation::Palette(Palette {
                entries: Vec::with_capacity(4),
                words: Vec::new(),
                bits: 0,
            });
        }
        let Representation::Palette(palette) = &mut self.data else {
            unreachable!("mixed output has a palette");
        };
        palette.entries.clear();
        let mut indices = [0_u16; SECTION_VOLUME];
        for (index, &id) in blocks.iter().enumerate() {
            let slot = match palette.entries.iter().position(|entry| entry.id == id) {
                Some(slot) => slot,
                None => {
                    palette.entries.push(PaletteEntry { id, count: 0 });
                    palette.entries.len() - 1
                }
            };
            palette.entries[slot].count += 1;
            indices[index] = slot as u16;
        }
        palette.bits = index_bits(palette.entries.len());
        palette.words.resize(word_count(palette.bits), 0);
        palette.words.fill(0);
        for (index, &slot) in indices.iter().enumerate() {
            write_index(&mut palette.words, palette.bits, index, usize::from(slot));
        }
        Ok(())
    }

    pub fn get(&self, position: LocalPos) -> VoxelId {
        self.at_index(position.index())
    }

    fn at_index(&self, index: usize) -> VoxelId {
        match &self.data {
            Representation::Empty => VoxelId::AIR,
            Representation::Uniform(id) => *id,
            Representation::Palette(palette) => {
                palette.entries[read_index(&palette.words, palette.bits, index)].id
            }
        }
    }

    /// Returns true only if the stored value changed. Repacking occurs only when
    /// the palette crosses a bit-width boundary, never on ordinary reads/writes.
    pub fn set(&mut self, position: LocalPos, id: VoxelId) -> bool {
        let index = position.index();
        match &mut self.data {
            Representation::Empty | Representation::Uniform(_) => {
                let old = self.at_index(index);
                if old == id {
                    return false;
                }
                let entries = vec![
                    PaletteEntry {
                        id: old,
                        count: (SECTION_VOLUME - 1) as u16,
                    },
                    PaletteEntry { id, count: 1 },
                ];
                let mut words = vec![0; word_count(1)];
                write_index(&mut words, 1, index, 1);
                self.data = Representation::Palette(Palette {
                    entries,
                    words,
                    bits: 1,
                });
            }
            Representation::Palette(palette) => {
                let old_slot = read_index(&palette.words, palette.bits, index);
                if palette.entries[old_slot].id == id {
                    return false;
                }
                let new_slot = if let Some(slot) =
                    palette.entries.iter().position(|entry| entry.id == id)
                {
                    slot
                } else if palette.entries[old_slot].count == 1 {
                    // Only this block references the old slot. Replace its ID in
                    // place: no dead entry, repack or 4097th palette slot can arise.
                    palette.entries[old_slot].id = id;
                    return true;
                } else if let Some(slot) = palette.entries.iter().position(|entry| entry.count == 0)
                {
                    palette.entries[slot].id = id;
                    slot
                } else {
                    let slot = palette.entries.len();
                    let new_bits = index_bits(slot + 1);
                    if new_bits != palette.bits {
                        palette.words = repack(&palette.words, palette.bits, new_bits);
                        palette.bits = new_bits;
                    }
                    palette.entries.push(PaletteEntry { id, count: 0 });
                    slot
                };
                palette.entries[old_slot].count -= 1;
                palette.entries[new_slot].count += 1;
                if usize::from(palette.entries[new_slot].count) == SECTION_VOLUME {
                    *self = Self::filled(id);
                } else {
                    write_index(&mut palette.words, palette.bits, index, new_slot);
                }
            }
        }
        true
    }

    pub fn fill(&mut self, id: VoxelId) -> bool {
        let unchanged = match self.data {
            Representation::Empty => id == VoxelId::AIR,
            Representation::Uniform(old) => old == id,
            Representation::Palette(_) => false,
        };
        if unchanged {
            return false;
        }
        *self = Self::filled(id);
        true
    }

    /// Local bulk edit: no coordinate hashing. Duplicate positions apply in order.
    /// Return the number of actual writes, not distinct positions.
    pub fn set_batch(&mut self, edits: &[(LocalPos, VoxelId)]) -> usize {
        edits
            .iter()
            .filter(|&&(position, id)| self.set(position, id))
            .count()
    }

    /// Explicit cold-path compaction removes dead entries and reduces width.
    /// Never changes values, and never runs implicitly every frame/write.
    pub fn compact(&mut self) -> bool {
        let Representation::Palette(palette) = &mut self.data else {
            return false;
        };
        let live = palette
            .entries
            .iter()
            .filter(|entry| entry.count != 0)
            .count();
        let has_unused = live != palette.entries.len();
        let has_capacity = palette.entries.capacity() != palette.entries.len();
        if !has_unused && !has_capacity {
            return false;
        }
        if has_unused {
            let mut mapping = vec![0; palette.entries.len()];
            let mut entries = Vec::with_capacity(live);
            for (old_slot, &entry) in palette.entries.iter().enumerate() {
                if entry.count != 0 {
                    mapping[old_slot] = entries.len();
                    entries.push(entry);
                }
            }
            let bits = index_bits(live);
            let mut words = vec![0; word_count(bits)];
            for index in 0..SECTION_VOLUME {
                let slot = read_index(&palette.words, palette.bits, index);
                write_index(&mut words, bits, index, mapping[slot]);
            }
            *palette = Palette {
                entries,
                words,
                bits,
            };
        } else {
            palette.entries.shrink_to_fit();
        }
        true
    }

    pub fn iter(&self) -> VoxelIter<'_> {
        VoxelIter {
            storage: self,
            index: 0,
        }
    }

    /// Materialize into caller-owned scratch memory (X, then Z, then Y).
    pub fn copy_voxels(&self, destination: &mut [VoxelId; SECTION_VOLUME]) {
        match self.data {
            Representation::Empty => destination.fill(VoxelId::AIR),
            Representation::Uniform(id) => destination.fill(id),
            Representation::Palette(_) => {
                for (output, id) in destination.iter_mut().zip(self.iter()) {
                    *output = id;
                }
            }
        }
    }

    pub fn memory_usage(&self) -> SectionMemory {
        let mut memory = SectionMemory {
            inline_bytes: size_of::<Self>(),
            heap_used_bytes: 0,
            heap_capacity_bytes: 0,
            palette_entries: 0,
            bits_per_index: 0,
        };
        if let Representation::Palette(palette) = &self.data {
            memory.heap_used_bytes = palette.entries.len() * size_of::<PaletteEntry>()
                + palette.words.len() * size_of::<u64>();
            memory.heap_capacity_bytes = palette.entries.capacity() * size_of::<PaletteEntry>()
                + palette.words.capacity() * size_of::<u64>();
            memory.palette_entries = palette.entries.len();
            memory.bits_per_index = palette.bits;
        }
        memory
    }
}

pub struct VoxelIter<'a> {
    storage: &'a SectionStorage,
    index: usize,
}

impl Iterator for VoxelIter<'_> {
    type Item = VoxelId;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index == SECTION_VOLUME {
            return None;
        }
        let value = self.storage.at_index(self.index);
        self.index += 1;
        Some(value)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = SECTION_VOLUME - self.index;
        (remaining, Some(remaining))
    }
}
impl ExactSizeIterator for VoxelIter<'_> {}
impl FusedIterator for VoxelIter<'_> {}

fn index_bits(entries: usize) -> u8 {
    (usize::BITS - (entries - 1).leading_zeros()) as u8
}
fn word_count(bits: u8) -> usize {
    SECTION_VOLUME * usize::from(bits) / 64
}

fn read_index(words: &[u64], bits: u8, index: usize) -> usize {
    let bit = index * usize::from(bits);
    let word = bit / 64;
    let offset = bit % 64;
    let mut value = words[word] >> offset;
    if offset + usize::from(bits) > 64 {
        value |= words[word + 1] << (64 - offset);
    }
    (value & ((1_u64 << bits) - 1)) as usize
}

fn write_index(words: &mut [u64], bits: u8, index: usize, value: usize) {
    let bit = index * usize::from(bits);
    let word = bit / 64;
    let offset = bit % 64;
    let mask = (1_u64 << bits) - 1;
    words[word] = (words[word] & !(mask << offset)) | ((value as u64) << offset);
    if offset + usize::from(bits) > 64 {
        let high_bits = offset + usize::from(bits) - 64;
        let high_mask = (1_u64 << high_bits) - 1;
        words[word + 1] = (words[word + 1] & !high_mask) | ((value as u64) >> (64 - offset));
    }
}

fn repack(old: &[u64], old_bits: u8, new_bits: u8) -> Vec<u64> {
    let mut words = vec![0; word_count(new_bits)];
    for index in 0..SECTION_VOLUME {
        write_index(
            &mut words,
            new_bits,
            index,
            read_index(old, old_bits, index),
        );
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bulk_replacement_reuses_buffers_and_rejects_invalid_input_atomically() {
        let mut section = SectionStorage::default();
        let blocks = std::array::from_fn::<_, SECTION_VOLUME, _>(|i| VoxelId::new((i % 8) as u32));
        section.replace_from_voxels(&blocks).unwrap();
        assert!(section.iter().eq(blocks));
        let capacity = section.memory_usage().heap_capacity_bytes;
        let replacement = blocks.map(|id| VoxelId::new(id.raw() + 100));
        section.replace_from_voxels(&replacement).unwrap();
        assert!(section.iter().eq(replacement));
        assert_eq!(section.memory_usage().heap_capacity_bytes, capacity);
        for invalid in [&[][..], &replacement[..4095]] {
            assert!(section.replace_from_voxels(invalid).is_err());
            assert!(section.iter().eq(replacement));
            assert_eq!(section.memory_usage().heap_capacity_bytes, capacity);
        }
        section
            .replace_from_voxels(&[VoxelId::new(u32::MAX); SECTION_VOLUME])
            .unwrap();
        assert_eq!(section.kind(), StorageKind::Uniform);
        assert_eq!(section.memory_usage().heap_capacity_bytes, 0);
        assert!(section.iter().all(|id| id.raw() == u32::MAX));
        section
            .replace_from_voxels(&[VoxelId::AIR; SECTION_VOLUME])
            .unwrap();
        assert_eq!(section.kind(), StorageKind::Empty);
    }

    #[test]
    fn empty_and_uniform_have_no_heap_and_copy_correctly() {
        for id in [VoxelId::AIR, VoxelId::new(1), VoxelId::new(3)] {
            let storage = SectionStorage::filled(id);
            assert_eq!(storage.memory_usage().heap_capacity_bytes, 0);
            assert_eq!(storage.iter().len(), 4096);
            assert!(storage.iter().all(|value| value == id));
            let mut blocks = [VoxelId::new(2); SECTION_VOLUME];
            storage.copy_voxels(&mut blocks);
            assert!(blocks.iter().all(|&value| value == id));
        }
        assert_eq!(SectionStorage::default().kind(), StorageKind::Empty);
    }

    #[test]
    fn single_change_promotes_and_restoring_collapses_without_data_loss() -> Result<(), VoxelError>
    {
        for base in [VoxelId::AIR, VoxelId::new(1)] {
            for index in [0, 63, 64, 2048, 4095] {
                let mut section = SectionStorage::filled(base);
                let position = LocalPos::from_index(index)?;
                assert!(section.set(position, VoxelId::new(3)));
                assert_eq!(section.memory_usage().bits_per_index, 1);
                for (i, value) in section.iter().enumerate() {
                    assert_eq!(value, if i == index { VoxelId::new(3) } else { base });
                }
                assert!(section.set(position, base));
                assert_ne!(section.kind(), StorageKind::Palette);
                assert_eq!(section.memory_usage().heap_capacity_bytes, 0);
            }
        }
        Ok(())
    }

    #[test]
    fn bit_packing_handles_every_width_and_word_boundary() {
        for bits in 1..=12 {
            let mask = (1_usize << bits) - 1;
            let mut words = vec![0; word_count(bits)];
            for i in 0..SECTION_VOLUME {
                write_index(&mut words, bits, i, (i * 37 + 7) & mask);
            }
            for i in 0..SECTION_VOLUME {
                assert_eq!(read_index(&words, bits, i), (i * 37 + 7) & mask);
            }
            for i in (0..SECTION_VOLUME).step_by(3) {
                write_index(&mut words, bits, i, 0);
            }
            for i in 0..SECTION_VOLUME {
                assert_eq!(
                    read_index(&words, bits, i),
                    if i % 3 == 0 { 0 } else { (i * 37 + 7) & mask }
                );
            }
        }
    }

    #[test]
    fn four_block_bulk_storage_uses_two_bits_and_round_trips() -> Result<(), VoxelError> {
        let blocks = std::array::from_fn::<_, SECTION_VOLUME, _>(|i| VoxelId::new((i % 4) as u32));
        let section = SectionStorage::from_voxels(&blocks)?;
        let memory = section.memory_usage();
        assert_eq!(memory.bits_per_index, 2);
        assert_eq!(memory.heap_capacity_bytes, 1024 + 32);
        assert!(section.iter().eq(blocks));
        let mut copied = [VoxelId::AIR; SECTION_VOLUME];
        section.copy_voxels(&mut copied);
        assert_eq!(copied, blocks);
        Ok(())
    }

    #[test]
    fn growing_through_all_palette_widths_preserves_4096_unique_values() -> Result<(), VoxelError> {
        let mut section = SectionStorage::default();
        for index in 0..SECTION_VOLUME {
            section.set(LocalPos::from_index(index)?, VoxelId::new(index as u32));
        }
        assert_eq!(section.memory_usage().bits_per_index, 12);
        for (i, id) in section.iter().enumerate() {
            assert_eq!(id.raw(), i as u32);
        }
        for index in 1..SECTION_VOLUME {
            section.set(LocalPos::from_index(index)?, VoxelId::AIR);
        }
        assert_eq!(section.kind(), StorageKind::Empty);
        assert_eq!(section.memory_usage().heap_capacity_bytes, 0);
        Ok(())
    }

    #[test]
    fn full_palette_replaces_unique_ids_without_growing_past_twelve_bits() -> Result<(), VoxelError>
    {
        let blocks = std::array::from_fn::<_, SECTION_VOLUME, _>(|i| VoxelId::new(i as u32));
        let mut section = SectionStorage::from_voxels(&blocks)?;
        let before = section.memory_usage();
        for index in 0..SECTION_VOLUME {
            let id = VoxelId::new(u32::MAX - index as u32);
            assert!(section.set(LocalPos::from_index(index)?, id));
            assert_eq!(section.memory_usage().bits_per_index, 12);
            assert_eq!(section.memory_usage().palette_entries, SECTION_VOLUME);
        }
        for (index, id) in section.iter().enumerate() {
            assert_eq!(id.raw(), u32::MAX - index as u32);
        }
        assert_eq!(section.memory_usage(), before);
        Ok(())
    }

    #[test]
    fn singleton_replacements_do_not_grow_the_palette() -> Result<(), VoxelError> {
        let mut section = SectionStorage::default();
        let p = LocalPos::from_index(65)?;
        section.set(p, VoxelId::new(1));
        for raw in 2..20_000 {
            section.set(p, VoxelId::new(raw));
        }
        assert_eq!(section.memory_usage().bits_per_index, 1);
        assert_eq!(section.memory_usage().palette_entries, 2);
        assert_eq!(section.get(p).raw(), 19_999);
        assert!(!section.compact());
        Ok(())
    }

    #[test]
    fn dead_palette_slot_is_reused_without_repacking() -> Result<(), VoxelError> {
        let mut reference =
            std::array::from_fn::<_, SECTION_VOLUME, _>(|i| VoxelId::new((i % 3) as u32));
        let mut section = SectionStorage::from_voxels(&reference)?;
        let before = section.memory_usage();
        for (index, id) in reference.iter_mut().enumerate() {
            if *id == VoxelId::new(2) {
                *id = VoxelId::new(1);
                section.set(LocalPos::from_index(index)?, *id);
            }
        }
        reference[0] = VoxelId::new(3);
        section.set(LocalPos::ZERO, reference[0]);
        assert_eq!(section.memory_usage(), before);
        assert!(section.iter().eq(reference));
        assert!(!section.compact());
        Ok(())
    }

    #[test]
    fn random_mutations_compaction_and_fill_match_uncompressed_reference() -> Result<(), VoxelError>
    {
        let mut section = SectionStorage::default();
        let mut reference = [VoxelId::AIR; SECTION_VOLUME];
        let mut random = 0x1234_5678_u64;
        for step in 0..80_000 {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            let index = (random as usize) % SECTION_VOLUME;
            let id = VoxelId::new(((random >> 32) % 19) as u32);
            assert_eq!(
                section.set(LocalPos::from_index(index)?, id),
                reference[index] != id
            );
            reference[index] = id;
            if step % 997 == 0 {
                section.compact();
                assert!(section.iter().eq(reference));
            }
        }
        assert!(section.iter().eq(reference));
        assert!(section.fill(VoxelId::new(1)));
        assert!(section.iter().all(|id| id == VoxelId::new(1)));
        assert!(!section.fill(VoxelId::new(1)));
        assert!(section.fill(VoxelId::AIR));
        assert_eq!(section.kind(), StorageKind::Empty);
        Ok(())
    }

    #[test]
    fn batch_order_noops_clones_and_iterator_exhaustion() -> Result<(), VoxelError> {
        let p = LocalPos::new(15, 15, 15)?;
        let mut section = SectionStorage::default();
        assert_eq!(
            section.set_batch(&[
                (p, VoxelId::new(1)),
                (p, VoxelId::new(1)),
                (p, VoxelId::new(2))
            ]),
            2
        );
        let clone = section.clone();
        section.set(p, VoxelId::new(3));
        assert_eq!(clone.get(p), VoxelId::new(2));
        let mut iter = clone.iter();
        for _ in 0..4096 {
            assert!(iter.next().is_some());
        }
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        Ok(())
    }

    #[test]
    fn bulk_construction_rejects_wrong_length_and_compresses_uniform() {
        assert!(matches!(
            SectionStorage::from_voxels(&[]),
            Err(VoxelError::InvalidBlockCount(0))
        ));
        assert!(SectionStorage::from_voxels(&vec![VoxelId::AIR; 4097]).is_err());
        assert_eq!(
            SectionStorage::from_voxels(&[VoxelId::new(1); SECTION_VOLUME])
                .unwrap()
                .kind(),
            StorageKind::Uniform
        );
    }

    #[test]
    fn removing_many_palette_entries_reduces_width_without_reordering_blocks()
    -> Result<(), VoxelError> {
        let mut reference =
            std::array::from_fn::<_, SECTION_VOLUME, _>(|i| VoxelId::new((i % 9) as u32));
        let mut section = SectionStorage::from_voxels(&reference)?;
        assert_eq!(section.memory_usage().bits_per_index, 4);
        for (index, id) in reference.iter_mut().enumerate() {
            if id.raw() > 1 {
                *id = VoxelId::new(1);
                section.set(LocalPos::from_index(index)?, *id);
            }
        }
        assert_eq!(section.memory_usage().bits_per_index, 4);
        assert!(section.compact());
        assert_eq!(section.memory_usage().bits_per_index, 1);
        assert_eq!(section.memory_usage().heap_capacity_bytes, 528);
        assert!(section.iter().eq(reference));
        // Remove the original slot 0 as well: uniform collapse needs no scan.
        for index in 0..SECTION_VOLUME {
            section.set(LocalPos::from_index(index)?, VoxelId::new(1));
        }
        assert_eq!(section.kind(), StorageKind::Uniform);
        assert_eq!(section.memory_usage().heap_capacity_bytes, 0);
        assert!(section.iter().all(|id| id == VoxelId::new(1)));
        Ok(())
    }
}
