//! Stock equipment set effects, from native-equipment-effects-catalog.json.
//! equipment-goldens.json retains and exercises every authored row.

use super::{EquipmentEffect, EquipmentSet};

pub const EQUIPMENT_SETS: [EquipmentSet; 7] = [
    EquipmentSet {
        name: "The Arcanus Spectrum Paraclosm",
        member_recipe_indices: &[0, 1, 2, 3, 4, 5],
        effects: &[EquipmentEffect { kind: 21, magnitude: 3.0, operator: 1, target: 0 }],
    },
    EquipmentSet {
        name: "Combinator's Coutrement",
        member_recipe_indices: &[6, 7, 8, 9, 10],
        effects: &[
            EquipmentEffect { kind: 37, magnitude: 0.0, operator: 0, target: 0 },
            EquipmentEffect { kind: 38, magnitude: 20.0, operator: 2, target: 0 },
        ],
    },
    EquipmentSet {
        name: "Pandimensional Bug-Master's Outfit",
        member_recipe_indices: &[11, 12, 13, 14, 15],
        effects: &[
            EquipmentEffect { kind: 26, magnitude: 0.0, operator: 0, target: 0 },
            EquipmentEffect { kind: 25, magnitude: 2.0, operator: 1, target: 11 },
        ],
    },
    EquipmentSet {
        name: "Tempest Kit",
        member_recipe_indices: &[16, 17, 18, 19],
        effects: &[
            EquipmentEffect { kind: 27, magnitude: 0.0, operator: 0, target: 0 },
            EquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 29 },
            EquipmentEffect { kind: 11, magnitude: -20.0, operator: 2, target: 2 },
        ],
    },
    EquipmentSet {
        name: "Burning Man",
        member_recipe_indices: &[20, 21],
        effects: &[EquipmentEffect { kind: 28, magnitude: 0.0, operator: 0, target: 0 }],
    },
    EquipmentSet {
        name: "Frostburn Jewels",
        member_recipe_indices: &[22, 23, 24],
        effects: &[
            EquipmentEffect { kind: 30, magnitude: 0.0, operator: 0, target: 0 },
            EquipmentEffect { kind: 13, magnitude: 10.0, operator: 2, target: 3 },
        ],
    },
    EquipmentSet {
        name: "Fete of Clay",
        member_recipe_indices: &[25, 26, 27, 28],
        effects: &[
            EquipmentEffect { kind: 29, magnitude: 0.0, operator: 0, target: 0 },
            EquipmentEffect { kind: 13, magnitude: 10.0, operator: 2, target: 4 },
        ],
    },
];
