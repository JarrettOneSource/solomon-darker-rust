//! Stock equipment and set effects, from native-equipment-effects-catalog.json.
//! equipment-goldens.json retains and exercises every authored row.

use super::{NativeEquipmentEffect, NativeEquipmentRecipe, NativeEquipmentSet};

pub const NATIVE_EQUIPMENT_RECIPES: [NativeEquipmentRecipe; 47] = [
    NativeEquipmentRecipe {
        name: "Pentaclostic Ring",
        description: "",
        effects: &[NativeEquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 11 }],
    },
    NativeEquipmentRecipe {
        name: "Arcanoric Robe",
        description: "",
        effects: &[
            NativeEquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 27 },
            NativeEquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 28 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Cosmofluxic Wand",
        description: "",
        effects: &[
            NativeEquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 49 },
            NativeEquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 51 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Theptoplasmar Amulet",
        description: "",
        effects: &[NativeEquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 21 }],
    },
    NativeEquipmentRecipe {
        name: "Synertauxic Ring",
        description: "",
        effects: &[NativeEquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 35 }],
    },
    NativeEquipmentRecipe {
        name: "Sublunarous Hat",
        description: "",
        effects: &[
            NativeEquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 45 },
            NativeEquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 41 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Combinator's Cap",
        description: "",
        effects: &[NativeEquipmentEffect { kind: 38, magnitude: 10.0, operator: 2, target: 0 }],
    },
    NativeEquipmentRecipe {
        name: "Combinator's Cape",
        description: "",
        effects: &[NativeEquipmentEffect { kind: 38, magnitude: 10.0, operator: 2, target: 0 }],
    },
    NativeEquipmentRecipe {
        name: "Combinator's Club",
        description: "",
        effects: &[NativeEquipmentEffect { kind: 39, magnitude: 0.0, operator: 0, target: 0 }],
    },
    NativeEquipmentRecipe {
        name: "Combinator's Choker",
        description: "",
        effects: &[
            NativeEquipmentEffect { kind: 38, magnitude: 5.0, operator: 2, target: 0 },
            NativeEquipmentEffect { kind: 9, magnitude: 5.0, operator: 2, target: 0 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Combinator's Circle",
        description: "",
        effects: &[
            NativeEquipmentEffect { kind: 38, magnitude: 5.0, operator: 2, target: 0 },
            NativeEquipmentEffect { kind: 9, magnitude: 5.0, operator: 2, target: 0 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Bug-Master's Cap",
        description: "",
        effects: &[NativeEquipmentEffect { kind: 25, magnitude: 1.0, operator: 0, target: 11 }],
    },
    NativeEquipmentRecipe {
        name: "Bug-Master's Robe",
        description: "",
        effects: &[NativeEquipmentEffect { kind: 25, magnitude: 1.0, operator: 0, target: 11 }],
    },
    NativeEquipmentRecipe {
        name: "Bug-Master's Wand",
        description: "",
        effects: &[NativeEquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 11 }],
    },
    NativeEquipmentRecipe {
        name: "Bug-Master's Loop",
        description: "",
        effects: &[NativeEquipmentEffect { kind: 25, magnitude: 1.0, operator: 0, target: 11 }],
    },
    NativeEquipmentRecipe {
        name: "Pan-Dimensional Strangler",
        description: "",
        effects: &[
            NativeEquipmentEffect { kind: 4, magnitude: 1.0, operator: 0, target: 11 },
            NativeEquipmentEffect { kind: 25, magnitude: 1.0, operator: 0, target: 11 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Cloudcover Hood",
        description: "",
        effects: &[NativeEquipmentEffect { kind: 25, magnitude: 10.0, operator: 0, target: 27 }],
    },
    NativeEquipmentRecipe {
        name: "Ozone Cape",
        description: "",
        effects: &[NativeEquipmentEffect { kind: 5, magnitude: 2.0, operator: 0, target: 30 }],
    },
    NativeEquipmentRecipe {
        name: "Lightning Rod",
        description: "",
        effects: &[NativeEquipmentEffect { kind: 25, magnitude: 50.0, operator: 2, target: 24 }],
    },
    NativeEquipmentRecipe {
        name: "Storm Choker",
        description: "",
        effects: &[NativeEquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 27 }],
    },
    NativeEquipmentRecipe {
        name: "Burning Hat",
        description: "",
        effects: &[
            NativeEquipmentEffect { kind: 25, magnitude: 5.0, operator: 0, target: 16 },
            NativeEquipmentEffect { kind: 2, magnitude: 10.0, operator: 2, target: 1 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Burning Robe",
        description: "",
        effects: &[
            NativeEquipmentEffect { kind: 25, magnitude: 2.0, operator: 1, target: 23 },
            NativeEquipmentEffect { kind: 2, magnitude: 10.0, operator: 2, target: 1 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Biting Ring",
        description: "",
        effects: &[NativeEquipmentEffect { kind: 2, magnitude: 10.0, operator: 2, target: 3 }],
    },
    NativeEquipmentRecipe {
        name: "Bitter Ring",
        description: "",
        effects: &[NativeEquipmentEffect { kind: 2, magnitude: 10.0, operator: 2, target: 3 }],
    },
    NativeEquipmentRecipe {
        name: "Glittering Amulet",
        description: "",
        effects: &[
            NativeEquipmentEffect { kind: 5, magnitude: 1.0, operator: 0, target: 39 },
            NativeEquipmentEffect { kind: 5, magnitude: 1.0, operator: 0, target: 32 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Potter's Apron",
        description: "",
        effects: &[NativeEquipmentEffect { kind: 22, magnitude: 50.0, operator: 2, target: 4 }],
    },
    NativeEquipmentRecipe {
        name: "Clayshaper's Ring",
        description: "",
        effects: &[NativeEquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 45 }],
    },
    NativeEquipmentRecipe {
        name: "Claybaker's Ring",
        description: "",
        effects: &[NativeEquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 45 }],
    },
    NativeEquipmentRecipe {
        name: "Kiln",
        description: "",
        effects: &[NativeEquipmentEffect { kind: 7, magnitude: 2.0, operator: 0, target: 16 }],
    },
    NativeEquipmentRecipe {
        name: "Obfuscate's Meddler",
        description: "An amulet, apparently forged by Conchiphus Obfuscate himself.  The runes read \"Interferenal.\"",
        effects: &[NativeEquipmentEffect { kind: 1, magnitude: 50.0, operator: 2, target: 0 }],
    },
    NativeEquipmentRecipe {
        name: "Karen You Scandalous Wench",
        description: "An amulet known to be forged by the great wizard Basculus.  Much has been written about the unusual name, with some speculating that Basculus finished work on the amulet immediately after an unhappy love affair.",
        effects: &[
            NativeEquipmentEffect { kind: 21, magnitude: 10.0, operator: 2, target: 0 },
            NativeEquipmentEffect { kind: 24, magnitude: 50.0, operator: 0, target: 0 },
            NativeEquipmentEffect { kind: 17, magnitude: 50.0, operator: 2, target: 0 },
            NativeEquipmentEffect { kind: 6, magnitude: 1.0, operator: 0, target: 5 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Poxproof",
        description: "An amulet clearly forged during the Great Healing Madness.",
        effects: &[NativeEquipmentEffect { kind: 20, magnitude: 100.0, operator: 2, target: 0 }],
    },
    NativeEquipmentRecipe {
        name: "Ethereal Choker",
        description: "Designed for those whose passion for Ether Magic surpasses their taste.",
        effects: &[
            NativeEquipmentEffect { kind: 5, magnitude: 2.0, operator: 0, target: 8 },
            NativeEquipmentEffect { kind: 5, magnitude: 2.0, operator: 0, target: 10 },
            NativeEquipmentEffect { kind: 5, magnitude: 2.0, operator: 0, target: 9 },
            NativeEquipmentEffect { kind: 11, magnitude: -15.0, operator: 2, target: 0 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Absolox's Boomstick",
        description: "Formerly the property of Archmage Garthus Absolox, who was overly enthusiastic about the Meteor spell.",
        effects: &[
            NativeEquipmentEffect { kind: 7, magnitude: 2.0, operator: 0, target: 16 },
            NativeEquipmentEffect { kind: 7, magnitude: 2.0, operator: 0, target: 40 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Staff of Dawn",
        description: "Forged by the wizard Gnoxis to achieve minimal functionality on mornings without coffee.  The imprints of hands have been squeezed into the wood at the gripping spots.",
        effects: &[NativeEquipmentEffect { kind: 6, magnitude: 2.0, operator: 0, target: 6 }],
    },
    NativeEquipmentRecipe {
        name: "Ringwall",
        description: "A simple ring, crafted to give unhealthy mages that extra staying power.",
        effects: &[NativeEquipmentEffect { kind: 4, magnitude: 2.0, operator: 0, target: 54 }],
    },
    NativeEquipmentRecipe {
        name: "Fleetfinger",
        description: "Forged to boost dexterity and allow mages to work faster, this ring was stolen immediately by Casanava Lancashire, history's greatest reputed lover.  The ring only resurfaced again recently, but frequently changes owners because of a tendency to fly off the hand in the heat of casting.",
        effects: &[NativeEquipmentEffect { kind: 12, magnitude: 100.0, operator: 2, target: 0 }],
    },
    NativeEquipmentRecipe {
        name: "Gritchenscorn",
        description: "Created by Blue Mage Wendrell to prove once and for all that a strong mind is worth more that a strong body.",
        effects: &[
            NativeEquipmentEffect { kind: 24, magnitude: 3.0, operator: 1, target: 0 },
            NativeEquipmentEffect { kind: 23, magnitude: -80.0, operator: 2, target: 0 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Mindblowing Ring",
        description: "When presented at the artificer's conclave, this ring garnered much interest because it emits a destructive blast (dealing damage depending on level) when the bearer levels up.  Concern was expressed that the presenter (level 47) was so close to levelling up that even the experience of speaking before a large group might do it, and an early lunch was declared.",
        effects: &[NativeEquipmentEffect { kind: 36, magnitude: 0.0, operator: 0, target: 0 }],
    },
    NativeEquipmentRecipe {
        name: "Smartest Ring",
        description: "The high level requirments of this ring indicate that it was not necessarily forged to assist in learning, but rather to allow those smug in their knowledge to be more smug.",
        effects: &[NativeEquipmentEffect { kind: 8, magnitude: 1.0, operator: 0, target: 0 }],
    },
    NativeEquipmentRecipe {
        name: "Yzmar's Handicap",
        description: "Annoyed at being accused of dueling with opponents below his magical grade, the wizard Yzmar designed this hat to provide a handicap to his challengers and silence accusations of advantage.  Sadly, Yzmar was disintegrated in his next duel.",
        effects: &[
            NativeEquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 64 },
            NativeEquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 56 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Qubar's Ether",
        description: "Part of Qubar's \"Elemental Boost\" collection.",
        effects: &[
            NativeEquipmentEffect { kind: 2, magnitude: 50.0, operator: 2, target: 0 },
            NativeEquipmentEffect { kind: 11, magnitude: -50.0, operator: 2, target: 0 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Qubar's Fire",
        description: "Part of Qubar's \"Elemental Boost\" collection.",
        effects: &[
            NativeEquipmentEffect { kind: 2, magnitude: 50.0, operator: 2, target: 1 },
            NativeEquipmentEffect { kind: 11, magnitude: -50.0, operator: 2, target: 1 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Qubar's Air",
        description: "Part of Qubar's \"Elemental Boost\" collection.",
        effects: &[
            NativeEquipmentEffect { kind: 2, magnitude: 50.0, operator: 2, target: 2 },
            NativeEquipmentEffect { kind: 11, magnitude: -50.0, operator: 2, target: 2 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Qubar's Water",
        description: "Part of Qubar's \"Elemental Boost\" collection.",
        effects: &[
            NativeEquipmentEffect { kind: 2, magnitude: 50.0, operator: 2, target: 3 },
            NativeEquipmentEffect { kind: 11, magnitude: -50.0, operator: 2, target: 3 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Qubar's Earth",
        description: "Part of Qubar's \"Elemental Boost\" collection.",
        effects: &[
            NativeEquipmentEffect { kind: 2, magnitude: 50.0, operator: 2, target: 4 },
            NativeEquipmentEffect { kind: 11, magnitude: -50.0, operator: 2, target: 4 },
        ],
    },
    NativeEquipmentRecipe {
        name: "Robe of Thaumic Unperturbability",
        description: "Woven as a defensive outfit for working with demons, this robe absorbs almost all magic in the vicinity.  The only known side effect is that your own magic is harmed as well.",
        effects: &[
            NativeEquipmentEffect { kind: 19, magnitude: 95.0, operator: 2, target: 0 },
            NativeEquipmentEffect { kind: 1, magnitude: -35.0, operator: 2, target: 0 },
            NativeEquipmentEffect { kind: 9, magnitude: -50.0, operator: 2, target: 0 },
            NativeEquipmentEffect { kind: 21, magnitude: -40.0, operator: 2, target: 0 },
        ],
    },
];

pub const NATIVE_EQUIPMENT_SETS: [NativeEquipmentSet; 7] = [
    NativeEquipmentSet {
        name: "The Arcanus Spectrum Paraclosm",
        member_recipe_indices: &[0, 1, 2, 3, 4, 5],
        effects: &[NativeEquipmentEffect { kind: 21, magnitude: 3.0, operator: 1, target: 0 }],
    },
    NativeEquipmentSet {
        name: "Combinator's Coutrement",
        member_recipe_indices: &[6, 7, 8, 9, 10],
        effects: &[
            NativeEquipmentEffect { kind: 37, magnitude: 0.0, operator: 0, target: 0 },
            NativeEquipmentEffect { kind: 38, magnitude: 20.0, operator: 2, target: 0 },
        ],
    },
    NativeEquipmentSet {
        name: "Pandimensional Bug-Master's Outfit",
        member_recipe_indices: &[11, 12, 13, 14, 15],
        effects: &[
            NativeEquipmentEffect { kind: 26, magnitude: 0.0, operator: 0, target: 0 },
            NativeEquipmentEffect { kind: 25, magnitude: 2.0, operator: 1, target: 11 },
        ],
    },
    NativeEquipmentSet {
        name: "Tempest Kit",
        member_recipe_indices: &[16, 17, 18, 19],
        effects: &[
            NativeEquipmentEffect { kind: 27, magnitude: 0.0, operator: 0, target: 0 },
            NativeEquipmentEffect { kind: 7, magnitude: 1.0, operator: 0, target: 29 },
            NativeEquipmentEffect { kind: 11, magnitude: -20.0, operator: 2, target: 2 },
        ],
    },
    NativeEquipmentSet {
        name: "Burning Man",
        member_recipe_indices: &[20, 21],
        effects: &[NativeEquipmentEffect { kind: 28, magnitude: 0.0, operator: 0, target: 0 }],
    },
    NativeEquipmentSet {
        name: "Frostburn Jewels",
        member_recipe_indices: &[22, 23, 24],
        effects: &[
            NativeEquipmentEffect { kind: 30, magnitude: 0.0, operator: 0, target: 0 },
            NativeEquipmentEffect { kind: 13, magnitude: 10.0, operator: 2, target: 3 },
        ],
    },
    NativeEquipmentSet {
        name: "Fete of Clay",
        member_recipe_indices: &[25, 26, 27, 28],
        effects: &[
            NativeEquipmentEffect { kind: 29, magnitude: 0.0, operator: 0, target: 0 },
            NativeEquipmentEffect { kind: 13, magnitude: 10.0, operator: 2, target: 4 },
        ],
    },
];
