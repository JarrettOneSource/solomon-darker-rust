//! Frame-scoped image materials keep animated filters out of the texture cache.

use std::collections::BTreeMap;

use egui::TextureId;
use solomon_graphics::{BlendMode, ColorFilter};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Material {
    pub image: u64,
    blend: u8,
    pub mask: bool,
    brightness: u32,
    saturation: u32,
}

impl Material {
    pub fn new(image: u64, blend: BlendMode, mask: bool, filter: ColorFilter) -> Self {
        Self {
            image,
            blend: match blend {
                BlendMode::Normal => 0,
                BlendMode::Add => 1,
                BlendMode::Multiply => 2,
            },
            mask,
            brightness: filter.brightness.to_bits(),
            saturation: filter.saturation.to_bits(),
        }
    }

    pub fn blend(self) -> BlendMode {
        match self.blend {
            1 => BlendMode::Add,
            2 => BlendMode::Multiply,
            _ => BlendMode::Normal,
        }
    }

    pub fn filter(self) -> ColorFilter {
        ColorFilter {
            brightness: f32::from_bits(self.brightness),
            saturation: f32::from_bits(self.saturation),
        }
    }
}

#[derive(Default)]
pub(super) struct Materials {
    ids: BTreeMap<Material, u64>,
    entries: Vec<Material>,
}

impl Materials {
    pub fn clear(&mut self) {
        self.ids.clear();
        self.entries.clear();
    }

    pub fn texture_id(&mut self, material: Material) -> TextureId {
        let next = self.entries.len() as u64;
        let id = *self.ids.entry(material).or_insert_with(|| {
            self.entries.push(material);
            next
        });
        TextureId::User(id)
    }

    pub fn get(&self, id: u64) -> Option<Material> {
        self.entries.get(usize::try_from(id).ok()?).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_filters_batch_identical_materials_without_aliasing_blend_or_mask() {
        let mut materials = Materials::default();
        let filtered = Material::new(
            7,
            BlendMode::Normal,
            false,
            ColorFilter { brightness: 1.4, saturation: 0.35 },
        );
        let id = materials.texture_id(filtered);
        assert_eq!(id, materials.texture_id(filtered));
        for material in [
            Material::new(8, BlendMode::Normal, false, filtered.filter()),
            Material::new(7, BlendMode::Add, false, filtered.filter()),
            Material::new(7, BlendMode::Normal, true, filtered.filter()),
            Material::new(7, BlendMode::Normal, false, ColorFilter::default()),
        ] {
            assert_ne!(id, materials.texture_id(material));
        }
        let TextureId::User(id) = id else { panic!("image material is a user texture") };
        assert_eq!(materials.get(id).unwrap().filter(), filtered.filter());
        assert_eq!(materials.get(id).unwrap().blend(), BlendMode::Normal);
    }

    #[test]
    fn a_new_frame_retires_animated_filter_descriptors() {
        let mut materials = Materials::default();
        for frame in 0..1000 {
            materials.clear();
            for gain in [1.0, 1.0 + frame as f32 * 0.001] {
                materials.texture_id(Material::new(
                    1,
                    BlendMode::Normal,
                    false,
                    ColorFilter { brightness: gain, saturation: 1.0 },
                ));
            }
            assert!(materials.entries.len() <= 2);
            assert!(materials.get(2).is_none());
        }
    }
}
