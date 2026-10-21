//! Prepared item art shared by the world, Inventory preview and material captures.
//! Texture validation happens before entering an infallible painter callback.

use std::rc::Rc;

use solomon_sim::modding::inventory::{ModItemContent, ModWearableSlot};

use super::*;
use crate::mod_presentation_assets::{ModPresentationAssets, ModWearableTextureFrames};

#[derive(Clone)]
pub struct WizardEquipment {
    pub(super) element: WizardElement,
    pub(super) appearance: PlayerRenderableEquipmentAppearance,
    pub(super) hat: Option<Rc<ModWearableTextureFrames>>,
    pub(super) robe: Option<Rc<ModWearableTextureFrames>>,
    pub(super) staff: Option<Rc<ModWearableTextureFrames>>,
}

impl WizardEquipment {
    pub fn prepare(
        renderer: &mut Renderer,
        element: WizardElement,
        equipment: &HubEquipmentState,
        mut assets: Option<&mut ModPresentationAssets>,
    ) -> Result<Self, String> {
        let appearance = player_living_equipment_appearance(element, equipment);
        let mut bank = |content: &ModItemContent, slot| {
            let assets =
                assets.as_deref_mut().ok_or("mod presentation textures are unavailable")?;
            let bank = assets
                .wearable(renderer, content)?
                .ok_or_else(|| format!("Missing mod {slot:?} wearable textures"))?;
            if bank.slot != slot {
                return Err(format!("Missing mod {slot:?} wearable textures"));
            }
            Ok(bank)
        };
        let hat = match &appearance.hat {
            Some(PlayerRenderableGarmentAppearance::Mod(garment)) => {
                Some(bank(&garment.content, ModWearableSlot::Hat)?)
            }
            _ => None,
        };
        let robe = match &appearance.robe {
            Some(PlayerRenderableGarmentAppearance::Mod(garment)) => {
                Some(bank(&garment.content, ModWearableSlot::Robe)?)
            }
            _ => None,
        };
        let staff = match &appearance.weapon {
            Some(PlayerRenderableWeaponAppearance::ModStaff(content)) => {
                Some(bank(content, ModWearableSlot::Staff)?)
            }
            _ => None,
        };
        Ok(Self { element, appearance, hat, robe, staff })
    }

    pub(super) fn starter(appearance: StarterEquipmentAppearance) -> Self {
        let garment = PlayerRenderableGarmentAppearance::Native(PlayerEquipmentTintedSelector {
            primary_tint: appearance.primary_tint,
            secondary_tint: appearance.secondary_tint,
            selector: 0,
        });
        Self {
            element: WizardElement::Ether,
            appearance: PlayerRenderableEquipmentAppearance {
                hat: Some(garment.clone()),
                robe: Some(garment),
                weapon: Some(PlayerRenderableWeaponAppearance::Native(PlayerWeaponAppearance {
                    kind: NativePlayerWeaponKind::Staff,
                    selector: 0,
                })),
            },
            hat: None,
            robe: None,
            staff: None,
        }
    }
}
