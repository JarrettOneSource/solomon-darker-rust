//! Stock equipment and set effects, from native-equipment-effects-catalog.json.
//! equipment-goldens.json retains and exercises every authored row.

use super::{EquipmentEffect, EquipmentSet, NativeEquipmentRecipe};

pub const EQUIPMENT_RECIPES: [NativeEquipmentRecipe; 47] = [
    NativeEquipmentRecipe {
        name: "Pentaclostic Ring",
        description: "",
        effects: &[EquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 11 }],
    },
    NativeEquipmentRecipe {
        name: "Arcanoric Robe",
        description: "",
        effects: &[
            EquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 27 },
            EquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 28 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Cosmofluxic Wand",
        description: "",
        effects: &[
            EquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 49 },
            EquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 51 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Theptoplasmar Amulet",
        description: "",
        effects: &[EquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 21 }],
    },
    NativeEquipmentRecipe {
        name: "Synertauxic Ring",
        description: "",
        effects: &[EquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 35 }],
    },
    NativeEquipmentRecipe {
        name: "Sublunarous Hat",
        description: "",
        effects: &[
            EquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 45 },
            EquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 41 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Combinator's Cap",
        description: "",
        effects: &[EquipmentEffect { kind: 38, magnitude: 10.0, operator: 2, target: 0 }],
    },
    NativeEquipmentRecipe {
        name: "Combinator's Cape",
        description: "",
        effects: &[EquipmentEffect { kind: 38, magnitude: 10.0, operator: 2, target: 0 }],
    },
    NativeEquipmentRecipe {
        name: "Combinator's Club",
        description: "",
        effects: &[EquipmentEffect { kind: 39, magnitude: 0.0, operator: 0, target: 0 }],
    },
    NativeEquipmentRecipe {
        name: "Combinator's Choker",
        description: "",
        effects: &[
            EquipmentEffect { kind: 38, magnitude: 5.0, operator: 2, target: 0 },
            EquipmentEffect { kind: 9, magnitude: 5.0, operator: 2, target: 0 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Combinator's Circle",
        description: "",
        effects: &[
            EquipmentEffect { kind: 38, magnitude: 5.0, operator: 2, target: 0 },
            EquipmentEffect { kind: 9, magnitude: 5.0, operator: 2, target: 0 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Bug-Master's Cap",
        description: "",
        effects: &[EquipmentEffect { kind: 25, magnitude: 1.0, operator: 0, target: 11 }],
    },
    NativeEquipmentRecipe {
        name: "Bug-Master's Robe",
        description: "",
        effects: &[EquipmentEffect { kind: 25, magnitude: 1.0, operator: 0, target: 11 }],
    },
    NativeEquipmentRecipe {
        name: "Bug-Master's Wand",
        description: "",
        effects: &[EquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 11 }],
    },
    NativeEquipmentRecipe {
        name: "Bug-Master's Loop",
        description: "",
        effects: &[EquipmentEffect { kind: 25, magnitude: 1.0, operator: 0, target: 11 }],
    },
    NativeEquipmentRecipe {
        name: "Pan-Dimensional Strangler",
        description: "",
        effects: &[
            EquipmentEffect { kind: 4, magnitude: 1.0, operator: 0, target: 11 },
            EquipmentEffect { kind: 25, magnitude: 1.0, operator: 0, target: 11 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Cloudcover Hood",
        description: "",
        effects: &[EquipmentEffect { kind: 25, magnitude: 10.0, operator: 0, target: 27 }],
    },
    NativeEquipmentRecipe {
        name: "Ozone Cape",
        description: "",
        effects: &[EquipmentEffect { kind: 5, magnitude: 2.0, operator: 0, target: 30 }],
    },
    NativeEquipmentRecipe {
        name: "Lightning Rod",
        description: "",
        effects: &[EquipmentEffect { kind: 25, magnitude: 50.0, operator: 2, target: 24 }],
    },
    NativeEquipmentRecipe {
        name: "Storm Choker",
        description: "",
        effects: &[EquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 27 }],
    },
    NativeEquipmentRecipe {
        name: "Burning Hat",
        description: "",
        effects: &[
            EquipmentEffect { kind: 25, magnitude: 5.0, operator: 0, target: 16 },
            EquipmentEffect { kind: 2, magnitude: 10.0, operator: 2, target: 1 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Burning Robe",
        description: "",
        effects: &[
            EquipmentEffect { kind: 25, magnitude: 2.0, operator: 1, target: 23 },
            EquipmentEffect { kind: 2, magnitude: 10.0, operator: 2, target: 1 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Biting Ring",
        description: "",
        effects: &[EquipmentEffect { kind: 2, magnitude: 10.0, operator: 2, target: 3 }],
    },
    NativeEquipmentRecipe {
        name: "Bitter Ring",
        description: "",
        effects: &[EquipmentEffect { kind: 2, magnitude: 10.0, operator: 2, target: 3 }],
    },
    NativeEquipmentRecipe {
        name: "Glittering Amulet",
        description: "",
        effects: &[
            EquipmentEffect { kind: 5, magnitude: 1.0, operator: 0, target: 39 },
            EquipmentEffect { kind: 5, magnitude: 1.0, operator: 0, target: 32 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Potter's Apron",
        description: "",
        effects: &[EquipmentEffect { kind: 22, magnitude: 50.0, operator: 2, target: 4 }],
    },
    NativeEquipmentRecipe {
        name: "Clayshaper's Ring",
        description: "",
        effects: &[EquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 45 }],
    },
    NativeEquipmentRecipe {
        name: "Claybaker's Ring",
        description: "",
        effects: &[EquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 45 }],
    },
    NativeEquipmentRecipe {
        name: "Kiln",
        description: "",
        effects: &[EquipmentEffect { kind: 7, magnitude: 2.0, operator: 0, target: 16 }],
    },
    NativeEquipmentRecipe {
        name: "Obfuscate's Meddler",
        description: "An amulet, apparently forged by Conchiphus Obfuscate himself.  The runes read \"Interferenal.\"",
        effects: &[EquipmentEffect { kind: 1, magnitude: 50.0, operator: 2, target: 0 }],
    },
    NativeEquipmentRecipe {
        name: "Karen You Scandalous Wench",
        description: "An amulet known to be forged by the great wizard Basculus.  Much has been written about the unusual name, with some speculating that Basculus finished work on the amulet immediately after an unhappy love affair.",
        effects: &[
            EquipmentEffect { kind: 21, magnitude: 10.0, operator: 2, target: 0 },
            EquipmentEffect { kind: 24, magnitude: 50.0, operator: 0, target: 0 },
            EquipmentEffect { kind: 17, magnitude: 50.0, operator: 2, target: 0 },
            EquipmentEffect { kind: 6, magnitude: 1.0, operator: 0, target: 5 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Poxproof",
        description: "An amulet clearly forged during the Great Healing Madness.",
        effects: &[EquipmentEffect { kind: 20, magnitude: 100.0, operator: 2, target: 0 }],
    },
    NativeEquipmentRecipe {
        name: "Ethereal Choker",
        description: "Designed for those whose passion for Ether Magic surpasses their taste.",
        effects: &[
            EquipmentEffect { kind: 5, magnitude: 2.0, operator: 0, target: 8 },
            EquipmentEffect { kind: 5, magnitude: 2.0, operator: 0, target: 10 },
            EquipmentEffect { kind: 5, magnitude: 2.0, operator: 0, target: 9 },
            EquipmentEffect { kind: 11, magnitude: -15.0, operator: 2, target: 0 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Absolox's Boomstick",
        description: "Formerly the property of Archmage Garthus Absolox, who was overly enthusiastic about the Meteor spell.",
        effects: &[
            EquipmentEffect { kind: 7, magnitude: 2.0, operator: 0, target: 16 },
            EquipmentEffect { kind: 7, magnitude: 2.0, operator: 0, target: 40 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Staff of Dawn",
        description: "Forged by the wizard Gnoxis to achieve minimal functionality on mornings without coffee.  The imprints of hands have been squeezed into the wood at the gripping spots.",
        effects: &[EquipmentEffect { kind: 6, magnitude: 2.0, operator: 0, target: 6 }],
    },
    NativeEquipmentRecipe {
        name: "Ringwall",
        description: "A simple ring, crafted to give unhealthy mages that extra staying power.",
        effects: &[EquipmentEffect { kind: 4, magnitude: 2.0, operator: 0, target: 54 }],
    },
    NativeEquipmentRecipe {
        name: "Fleetfinger",
        description: "Forged to boost dexterity and allow mages to work faster, this ring was stolen immediately by Casanava Lancashire, history's greatest reputed lover.  The ring only resurfaced again recently, but frequently changes owners because of a tendency to fly off the hand in the heat of casting.",
        effects: &[EquipmentEffect { kind: 12, magnitude: 100.0, operator: 2, target: 0 }],
    },
    NativeEquipmentRecipe {
        name: "Gritchenscorn",
        description: "Created by Blue Mage Wendrell to prove once and for all that a strong mind is worth more that a strong body.",
        effects: &[
            EquipmentEffect { kind: 24, magnitude: 3.0, operator: 1, target: 0 },
            EquipmentEffect { kind: 23, magnitude: -80.0, operator: 2, target: 0 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Mindblowing Ring",
        description: "When presented at the artificer's conclave, this ring garnered much interest because it emits a destructive blast (dealing damage depending on level) when the bearer levels up.  Concern was expressed that the presenter (level 47) was so close to levelling up that even the experience of speaking before a large group might do it, and an early lunch was declared.",
        effects: &[EquipmentEffect { kind: 36, magnitude: 0.0, operator: 0, target: 0 }],
    },
    NativeEquipmentRecipe {
        name: "Smartest Ring",
        description: "The high level requirments of this ring indicate that it was not necessarily forged to assist in learning, but rather to allow those smug in their knowledge to be more smug.",
        effects: &[EquipmentEffect { kind: 8, magnitude: 1.0, operator: 0, target: 0 }],
    },
    NativeEquipmentRecipe {
        name: "Yzmar's Handicap",
        description: "Annoyed at being accused of dueling with opponents below his magical grade, the wizard Yzmar designed this hat to provide a handicap to his challengers and silence accusations of advantage.  Sadly, Yzmar was disintegrated in his next duel.",
        effects: &[
            EquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 64 },
            EquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 56 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Qubar's Ether",
        description: "Part of Qubar's \"Elemental Boost\" collection.",
        effects: &[
            EquipmentEffect { kind: 2, magnitude: 50.0, operator: 2, target: 0 },
            EquipmentEffect { kind: 11, magnitude: -50.0, operator: 2, target: 0 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Qubar's Fire",
        description: "Part of Qubar's \"Elemental Boost\" collection.",
        effects: &[
            EquipmentEffect { kind: 2, magnitude: 50.0, operator: 2, target: 1 },
            EquipmentEffect { kind: 11, magnitude: -50.0, operator: 2, target: 1 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Qubar's Air",
        description: "Part of Qubar's \"Elemental Boost\" collection.",
        effects: &[
            EquipmentEffect { kind: 2, magnitude: 50.0, operator: 2, target: 2 },
            EquipmentEffect { kind: 11, magnitude: -50.0, operator: 2, target: 2 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Qubar's Water",
        description: "Part of Qubar's \"Elemental Boost\" collection.",
        effects: &[
            EquipmentEffect { kind: 2, magnitude: 50.0, operator: 2, target: 3 },
            EquipmentEffect { kind: 11, magnitude: -50.0, operator: 2, target: 3 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Qubar's Earth",
        description: "Part of Qubar's \"Elemental Boost\" collection.",
        effects: &[
            EquipmentEffect { kind: 2, magnitude: 50.0, operator: 2, target: 4 },
            EquipmentEffect { kind: 11, magnitude: -50.0, operator: 2, target: 4 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Robe of Thaumic Unperturbability",
        description: "Woven as a defensive outfit for working with demons, this robe absorbs almost all magic in the vicinity.  The only known side effect is that your own magic is harmed as well.",
        effects: &[
            EquipmentEffect { kind: 19, magnitude: 95.0, operator: 2, target: 0 },
            EquipmentEffect { kind: 1, magnitude: -35.0, operator: 2, target: 0 },
            EquipmentEffect { kind: 9, magnitude: -50.0, operator: 2, target: 0 },
            EquipmentEffect { kind: 21, magnitude: -40.0, operator: 2, target: 0 },
        ],
    },
];

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
