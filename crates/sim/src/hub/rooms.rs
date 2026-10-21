//! Private College room geometry and authored placements.
//!
//! Reference: `Website/frontend/src/game/core-kernels/hub-private-room-layout.ts`;
//! `renderer/hub-private-room-presentation.ts` supplies the flame anchors and flicker.
//! The segment coordinates below already include each table-to-world offset, exactly once.
//! `tests/website_goldens.rs` replays `hub-rooms-goldens.json`.

use super::regions::HubRegion;
use super::{HubSegment, PLAYER_CHARACTER_RADIUS, is_traversable_against_segments};
use crate::Vec2;
use crate::float::atan2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubRoomActor {
    pub id: &'static str,
    pub registration_ordinal: usize,
    pub position: Vec2,
    pub radius: f64,
    pub visual_position: Vec2,
    pub painter_y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HubRoomPropKind {
    Portrait,
    RoomFrame,
    RoomLayer,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubRoomPropVisual {
    pub kind: HubRoomPropKind,
    pub frame_index: usize,
    pub position: Vec2,
    pub painter_y: f64,
    pub marker: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubRoomProp {
    pub id: &'static str,
    pub registration_ordinal: usize,
    pub position: Vec2,
    pub radius: f64,
    pub visual: Option<HubRoomPropVisual>,
}

/// Mortuary collision table, translated by [27, 57] world units.
pub const MORTUARY_SEGMENTS: [HubSegment; 11] = [
    HubSegment::new(909.0, 965.0, 893.0, 411.0),
    HubSegment::new(893.0, 411.0, 835.0, 403.0),
    HubSegment::new(835.0, 403.0, 744.0, 340.0),
    HubSegment::new(744.0, 340.0, 721.0, 298.0),
    HubSegment::new(721.0, 298.0, 720.0, 271.0),
    HubSegment::new(720.0, 271.0, 302.0, 270.0),
    HubSegment::new(302.0, 270.0, 304.0, 298.0),
    HubSegment::new(304.0, 298.0, 280.0, 339.0),
    HubSegment::new(280.0, 339.0, 191.0, 403.0),
    HubSegment::new(191.0, 403.0, 132.0, 417.0),
    HubSegment::new(132.0, 417.0, 112.0, 973.0),
];

const MORTUARY_ACTORS: [HubRoomActor; 1] = [HubRoomActor {
    id: "memorator",
    registration_ordinal: 15,
    position: Vec2::new(628.0, 770.0),
    radius: 25.0,
    visual_position: Vec2::new(628.0, 770.0),
    painter_y: 770.0,
}];

const MORTUARY_PROPS: [HubRoomProp; 10] = [
    HubRoomProp {
        id: "painting-0",
        registration_ordinal: 26,
        position: Vec2::new(512.0, 695.0),
        radius: 40.0,
        visual: Some(HubRoomPropVisual {
            kind: HubRoomPropKind::Portrait,
            frame_index: 0,
            position: Vec2::new(512.0, 702.0),
            painter_y: 695.0,
            marker: false,
        }),
    },
    HubRoomProp {
        id: "painting-1",
        registration_ordinal: 27,
        position: Vec2::new(350.0, 681.0),
        radius: 40.0,
        visual: Some(HubRoomPropVisual {
            kind: HubRoomPropKind::Portrait,
            frame_index: 1,
            position: Vec2::new(350.0, 688.0),
            painter_y: 681.0,
            marker: true,
        }),
    },
    HubRoomProp {
        id: "painting-2",
        registration_ordinal: 28,
        position: Vec2::new(673.0, 681.0),
        radius: 40.0,
        visual: Some(HubRoomPropVisual {
            kind: HubRoomPropKind::Portrait,
            frame_index: 2,
            position: Vec2::new(673.0, 688.0),
            painter_y: 681.0,
            marker: true,
        }),
    },
    HubRoomProp {
        id: "painting-3",
        registration_ordinal: 29,
        position: Vec2::new(744.0, 538.0),
        radius: 40.0,
        visual: Some(HubRoomPropVisual {
            kind: HubRoomPropKind::Portrait,
            frame_index: 3,
            position: Vec2::new(744.0, 545.0),
            painter_y: 538.0,
            marker: true,
        }),
    },
    HubRoomProp {
        id: "painting-4",
        registration_ordinal: 30,
        position: Vec2::new(590.0, 538.0),
        radius: 40.0,
        visual: Some(HubRoomPropVisual {
            kind: HubRoomPropKind::Portrait,
            frame_index: 4,
            position: Vec2::new(590.0, 545.0),
            painter_y: 538.0,
            marker: false,
        }),
    },
    HubRoomProp {
        id: "painting-5",
        registration_ordinal: 31,
        position: Vec2::new(434.0, 538.0),
        radius: 40.0,
        visual: Some(HubRoomPropVisual {
            kind: HubRoomPropKind::Portrait,
            frame_index: 5,
            position: Vec2::new(434.0, 545.0),
            painter_y: 538.0,
            marker: true,
        }),
    },
    HubRoomProp {
        id: "painting-6",
        registration_ordinal: 32,
        position: Vec2::new(279.0, 538.0),
        radius: 40.0,
        visual: Some(HubRoomPropVisual {
            kind: HubRoomPropKind::Portrait,
            frame_index: 6,
            position: Vec2::new(279.0, 545.0),
            painter_y: 538.0,
            marker: true,
        }),
    },
    HubRoomProp {
        id: "painting-7",
        registration_ordinal: 33,
        position: Vec2::new(354.0, 398.0),
        radius: 40.0,
        visual: Some(HubRoomPropVisual {
            kind: HubRoomPropKind::Portrait,
            frame_index: 7,
            position: Vec2::new(354.0, 405.0),
            painter_y: 398.0,
            marker: false,
        }),
    },
    HubRoomProp {
        id: "painting-8",
        registration_ordinal: 34,
        position: Vec2::new(512.0, 398.0),
        radius: 40.0,
        visual: Some(HubRoomPropVisual {
            kind: HubRoomPropKind::Portrait,
            frame_index: 8,
            position: Vec2::new(512.0, 405.0),
            painter_y: 398.0,
            marker: false,
        }),
    },
    HubRoomProp {
        id: "painting-9",
        registration_ordinal: 35,
        position: Vec2::new(670.0, 398.0),
        radius: 40.0,
        visual: Some(HubRoomPropVisual {
            kind: HubRoomPropKind::Portrait,
            frame_index: 9,
            position: Vec2::new(670.0, 405.0),
            painter_y: 398.0,
            marker: true,
        }),
    },
];

const MORTUARY_FLAMES: [Vec2; 50] = [
    Vec2::new(190.5, 307.5),
    Vec2::new(196.5, 303.5),
    Vec2::new(205.5, 296.5),
    Vec2::new(213.5, 290.5),
    Vec2::new(221.5, 284.5),
    Vec2::new(236.5, 273.5),
    Vec2::new(242.5, 268.5),
    Vec2::new(251.5, 262.5),
    Vec2::new(274.5, 245.5),
    Vec2::new(263.5, 234.5),
    Vec2::new(255.5, 240.5),
    Vec2::new(247.5, 245.5),
    Vec2::new(240.5, 251.5),
    Vec2::new(225.5, 260.5),
    Vec2::new(202.5, 279.5),
    Vec2::new(195.5, 285.5),
    Vec2::new(167.5, 286.5),
    Vec2::new(175.5, 279.5),
    Vec2::new(192.5, 267.5),
    Vec2::new(213.5, 252.5),
    Vec2::new(222.5, 245.5),
    Vec2::new(236.5, 234.5),
    Vec2::new(245.5, 228.5),
    Vec2::new(251.5, 224.5),
    Vec2::new(748.5, 246.5),
    Vec2::new(757.5, 252.5),
    Vec2::new(766.5, 257.5),
    Vec2::new(773.5, 263.5),
    Vec2::new(789.5, 274.5),
    Vec2::new(812.5, 290.5),
    Vec2::new(819.5, 296.5),
    Vec2::new(829.5, 301.5),
    Vec2::new(837.5, 305.5),
    Vec2::new(847.5, 294.5),
    Vec2::new(839.5, 290.5),
    Vec2::new(830.5, 284.5),
    Vec2::new(823.5, 279.5),
    Vec2::new(814.5, 273.5),
    Vec2::new(807.5, 267.5),
    Vec2::new(798.5, 263.5),
    Vec2::new(792.5, 257.5),
    Vec2::new(784.5, 252.5),
    Vec2::new(775.5, 246.5),
    Vec2::new(760.5, 236.5),
    Vec2::new(769.5, 223.5),
    Vec2::new(777.5, 229.5),
    Vec2::new(802.5, 245.5),
    Vec2::new(817.5, 256.5),
    Vec2::new(832.5, 267.5),
    Vec2::new(841.5, 271.5),
];

/// Library collision table, translated by [16, 102.5] world units.
pub const LIBRARY_SEGMENTS: [HubSegment; 27] = [
    HubSegment::new(945.0, 885.5, 937.0, 644.5),
    HubSegment::new(937.0, 644.5, 908.0, 631.5),
    HubSegment::new(908.0, 631.5, 828.0, 631.5),
    HubSegment::new(828.0, 631.5, 726.0, 617.5),
    HubSegment::new(726.0, 617.5, 675.0, 614.5),
    HubSegment::new(675.0, 614.5, 599.0, 623.5),
    HubSegment::new(599.0, 623.5, 598.0, 639.5),
    HubSegment::new(598.0, 639.5, 425.0, 637.5),
    HubSegment::new(425.0, 637.5, 423.0, 618.5),
    HubSegment::new(423.0, 618.5, 86.0, 614.5),
    HubSegment::new(86.0, 614.5, 79.0, 889.5),
    HubSegment::new(79.0, 887.5, 392.0, 886.5),
    HubSegment::new(392.0, 886.5, 392.0, 1008.5),
    HubSegment::new(946.0, 884.5, 629.0, 883.5),
    HubSegment::new(629.0, 883.5, 628.0, 1000.5),
    HubSegment::new(842.0, 773.5, 684.0, 773.5),
    HubSegment::new(684.0, 773.5, 684.0, 719.5),
    HubSegment::new(684.0, 719.5, 840.0, 719.5),
    HubSegment::new(840.0, 719.5, 840.0, 770.5),
    HubSegment::new(336.0, 719.5, 336.0, 669.5),
    HubSegment::new(336.0, 669.5, 182.0, 669.5),
    HubSegment::new(182.0, 669.5, 182.0, 719.5),
    HubSegment::new(182.0, 719.5, 336.0, 719.5),
    HubSegment::new(317.0, 826.5, 317.0, 776.5),
    HubSegment::new(317.0, 776.5, 161.0, 776.5),
    HubSegment::new(161.0, 776.5, 161.0, 826.5),
    HubSegment::new(161.0, 826.5, 317.0, 826.5),
];

const LIBRARY_ACTORS: [HubRoomActor; 2] = [
    HubRoomActor {
        id: "librarian",
        registration_ordinal: 40,
        position: Vec2::new(512.0, 595.0),
        radius: 55.0,
        visual_position: Vec2::new(512.0, 538.0),
        painter_y: 595.0,
    },
    HubRoomActor {
        id: "dowser",
        registration_ordinal: 41,
        position: Vec2::new(900.0, 642.5),
        radius: 25.0,
        visual_position: Vec2::new(900.0, 642.5),
        painter_y: 642.5,
    },
];

const LIBRARY_PROPS: [HubRoomProp; 4] = [
    HubRoomProp {
        id: "library-prop-0",
        registration_ordinal: 36,
        position: Vec2::new(239.5, 788.0),
        radius: 40.0,
        visual: Some(HubRoomPropVisual {
            kind: HubRoomPropKind::RoomFrame,
            frame_index: 0,
            position: Vec2::new(0.0, 0.0),
            painter_y: 788.0,
            marker: false,
        }),
    },
    HubRoomProp {
        id: "library-prop-1",
        registration_ordinal: 37,
        position: Vec2::new(258.5, 678.5),
        radius: 40.0,
        visual: Some(HubRoomPropVisual {
            kind: HubRoomPropKind::RoomFrame,
            frame_index: 1,
            position: Vec2::new(0.0, 0.0),
            painter_y: 678.5,
            marker: false,
        }),
    },
    HubRoomProp {
        id: "library-prop-2",
        registration_ordinal: 38,
        position: Vec2::new(762.0, 732.5),
        radius: 40.0,
        visual: Some(HubRoomPropVisual {
            kind: HubRoomPropKind::RoomFrame,
            frame_index: 2,
            position: Vec2::new(0.0, 0.0),
            painter_y: 732.5,
            marker: false,
        }),
    },
    HubRoomProp {
        id: "library-prop-3",
        registration_ordinal: 39,
        position: Vec2::new(831.0, 620.5),
        radius: 40.0,
        visual: None,
    },
];

const LIBRARY_FLAMES: [Vec2; 17] = [
    Vec2::new(464.0, 549.5),
    Vec2::new(539.0, 553.5),
    Vec2::new(566.0, 544.5),
    Vec2::new(917.0, 465.5),
    Vec2::new(920.0, 481.5),
    Vec2::new(923.0, 640.5),
    Vec2::new(925.0, 655.5),
    Vec2::new(801.0, 690.5),
    Vec2::new(729.0, 705.5),
    Vec2::new(289.0, 745.5),
    Vec2::new(225.0, 755.5),
    Vec2::new(292.0, 653.5),
    Vec2::new(220.0, 637.5),
    Vec2::new(106.0, 465.5),
    Vec2::new(104.0, 481.5),
    Vec2::new(101.0, 639.5),
    Vec2::new(99.0, 656.5),
];

/// Storeroom collision table, translated by [0, 72.5] world units.
pub const STOREROOM_SEGMENTS: [HubSegment; 34] = [
    HubSegment::new(586.0, 790.5, 587.0, 686.5),
    HubSegment::new(587.0, 686.5, 981.0, 686.5),
    HubSegment::new(981.0, 686.5, 975.0, 580.5),
    HubSegment::new(975.0, 580.5, 987.0, 440.5),
    HubSegment::new(987.0, 440.5, 962.0, 432.5),
    HubSegment::new(962.0, 432.5, 962.0, 233.5),
    HubSegment::new(962.0, 233.5, 849.0, 228.5),
    HubSegment::new(849.0, 228.5, 628.0, 244.5),
    HubSegment::new(628.0, 244.5, 445.0, 236.5),
    HubSegment::new(445.0, 236.5, 289.0, 241.5),
    HubSegment::new(289.0, 241.5, 193.0, 230.5),
    HubSegment::new(193.0, 230.5, 118.0, 247.5),
    HubSegment::new(118.0, 247.5, 112.0, 416.5),
    HubSegment::new(112.0, 416.5, 87.0, 548.5),
    HubSegment::new(87.0, 548.5, 89.0, 689.5),
    HubSegment::new(89.0, 689.5, 485.0, 689.5),
    HubSegment::new(485.0, 689.5, 485.0, 786.5),
    HubSegment::new(895.0, 597.5, 178.0, 597.5),
    HubSegment::new(178.0, 597.5, 175.0, 555.5),
    HubSegment::new(175.0, 555.5, 895.0, 551.5),
    HubSegment::new(895.0, 551.5, 895.0, 597.5),
    HubSegment::new(887.0, 475.5, 174.0, 475.5),
    HubSegment::new(174.0, 475.5, 174.0, 439.5),
    HubSegment::new(174.0, 439.5, 887.0, 439.5),
    HubSegment::new(887.0, 439.5, 887.0, 475.5),
    HubSegment::new(886.0, 368.5, 637.0, 363.5),
    HubSegment::new(637.0, 363.5, 423.0, 362.5),
    HubSegment::new(423.0, 362.5, 279.0, 381.5),
    HubSegment::new(279.0, 381.5, 178.0, 377.5),
    HubSegment::new(178.0, 377.5, 183.0, 326.5),
    HubSegment::new(183.0, 326.5, 421.0, 327.5),
    HubSegment::new(421.0, 327.5, 773.0, 331.5),
    HubSegment::new(773.0, 331.5, 881.0, 325.5),
    HubSegment::new(881.0, 325.5, 886.0, 368.5),
];

const STOREROOM_ACTORS: [HubRoomActor; 0] = [];

const STOREROOM_PROPS: [HubRoomProp; 3] = [
    HubRoomProp {
        id: "storeroom-prop-0",
        registration_ordinal: 42,
        position: Vec2::new(538.0, 324.0),
        radius: 40.0,
        visual: Some(HubRoomPropVisual {
            kind: HubRoomPropKind::RoomFrame,
            frame_index: 0,
            position: Vec2::new(0.0, 0.0),
            painter_y: 324.0,
            marker: false,
        }),
    },
    HubRoomProp {
        id: "storeroom-prop-1",
        registration_ordinal: 43,
        position: Vec2::new(537.5, 434.0),
        radius: 40.0,
        visual: Some(HubRoomPropVisual {
            kind: HubRoomPropKind::RoomFrame,
            frame_index: 1,
            position: Vec2::new(0.0, 0.0),
            painter_y: 434.0,
            marker: false,
        }),
    },
    HubRoomProp {
        id: "storeroom-prop-2",
        registration_ordinal: 44,
        position: Vec2::new(536.0, 542.5),
        radius: 40.0,
        visual: Some(HubRoomPropVisual {
            kind: HubRoomPropKind::RoomFrame,
            frame_index: 2,
            position: Vec2::new(0.0, 0.0),
            painter_y: 542.5,
            marker: false,
        }),
    },
];

const STOREROOM_FLAMES: [Vec2; 9] = [
    Vec2::new(588.0, 489.5),
    Vec2::new(561.0, 478.5),
    Vec2::new(421.0, 518.5),
    Vec2::new(391.0, 536.5),
    Vec2::new(337.0, 303.5),
    Vec2::new(391.0, 285.5),
    Vec2::new(570.0, 293.5),
    Vec2::new(611.0, 284.5),
    Vec2::new(831.0, 419.5),
];

/// Office collision table, translated by [102.5, 102.5] world units.
pub const OFFICE_SEGMENTS: [HubSegment; 48] = [
    HubSegment::new(600.5, 972.5, 598.5, 921.5),
    HubSegment::new(598.5, 921.5, 646.5, 851.5),
    HubSegment::new(646.5, 851.5, 800.5, 844.5),
    HubSegment::new(800.5, 844.5, 854.5, 801.5),
    HubSegment::new(854.5, 801.5, 853.5, 702.5),
    HubSegment::new(853.5, 702.5, 911.5, 702.5),
    HubSegment::new(911.5, 702.5, 910.5, 584.5),
    HubSegment::new(910.5, 584.5, 854.5, 585.5),
    HubSegment::new(854.5, 585.5, 802.5, 576.5),
    HubSegment::new(802.5, 576.5, 773.5, 553.5),
    HubSegment::new(773.5, 553.5, 735.5, 551.5),
    HubSegment::new(735.5, 551.5, 695.5, 515.5),
    HubSegment::new(695.5, 515.5, 654.5, 516.5),
    HubSegment::new(654.5, 516.5, 648.5, 540.5),
    HubSegment::new(648.5, 540.5, 625.5, 551.5),
    HubSegment::new(625.5, 551.5, 575.5, 556.5),
    HubSegment::new(575.5, 556.5, 549.5, 532.5),
    HubSegment::new(549.5, 532.5, 546.5, 510.5),
    HubSegment::new(546.5, 510.5, 483.5, 509.5),
    HubSegment::new(483.5, 509.5, 472.5, 539.5),
    HubSegment::new(472.5, 539.5, 443.5, 538.5),
    HubSegment::new(443.5, 538.5, 418.5, 531.5),
    HubSegment::new(418.5, 531.5, 368.5, 499.5),
    HubSegment::new(368.5, 499.5, 316.5, 529.5),
    HubSegment::new(316.5, 529.5, 287.5, 508.5),
    HubSegment::new(287.5, 508.5, 270.5, 513.5),
    HubSegment::new(270.5, 513.5, 270.5, 528.5),
    HubSegment::new(270.5, 528.5, 232.5, 539.5),
    HubSegment::new(232.5, 539.5, 182.5, 520.5),
    HubSegment::new(182.5, 520.5, 176.5, 581.5),
    HubSegment::new(176.5, 581.5, 112.5, 584.5),
    HubSegment::new(112.5, 584.5, 115.5, 692.5),
    HubSegment::new(115.5, 692.5, 167.5, 699.5),
    HubSegment::new(167.5, 699.5, 172.5, 809.5),
    HubSegment::new(172.5, 809.5, 225.5, 855.5),
    HubSegment::new(225.5, 855.5, 365.5, 860.5),
    HubSegment::new(365.5, 860.5, 416.5, 919.5),
    HubSegment::new(416.5, 919.5, 411.5, 979.5),
    HubSegment::new(450.5, 741.5, 589.5, 741.5),
    HubSegment::new(589.5, 741.5, 627.5, 731.5),
    HubSegment::new(627.5, 731.5, 638.5, 708.5),
    HubSegment::new(638.5, 708.5, 638.5, 676.5),
    HubSegment::new(638.5, 676.5, 625.5, 644.5),
    HubSegment::new(625.5, 644.5, 414.5, 644.5),
    HubSegment::new(414.5, 644.5, 402.5, 673.5),
    HubSegment::new(402.5, 673.5, 402.5, 710.5),
    HubSegment::new(402.5, 710.5, 416.5, 733.5),
    HubSegment::new(416.5, 733.5, 451.5, 741.5),
];

const OFFICE_ACTORS: [HubRoomActor; 1] = [HubRoomActor {
    id: "arch-chancellor",
    registration_ordinal: 46,
    position: Vec2::new(514.0, 467.0),
    radius: 55.0,
    visual_position: Vec2::new(518.0, 412.0),
    painter_y: 467.0,
}];

const OFFICE_PROPS: [HubRoomProp; 1] = [HubRoomProp {
    id: "office-prop-0",
    registration_ordinal: 45,
    position: Vec2::new(517.5, 681.0),
    radius: 40.0,
    visual: Some(HubRoomPropVisual {
        kind: HubRoomPropKind::RoomLayer,
        frame_index: 0,
        position: Vec2::new(0.0, 0.0),
        painter_y: 681.0,
        marker: false,
    }),
}];

const OFFICE_FLAMES: [Vec2; 7] = [
    Vec2::new(588.5, 448.5),
    Vec2::new(547.5, 422.5),
    Vec2::new(507.5, 424.5),
    Vec2::new(445.5, 437.5),
    Vec2::new(324.5, 434.5),
    Vec2::new(293.5, 446.5),
    Vec2::new(317.5, 470.5),
];

impl HubRegion {
    /// The same swept-segment geometry in every Hub region.
    #[must_use]
    pub const fn segments(self) -> &'static [HubSegment] {
        match self {
            Self::Courtyard => &super::HUB_COURTYARD_SEGMENTS,
            Self::Mortuary => &MORTUARY_SEGMENTS,
            Self::Library => &LIBRARY_SEGMENTS,
            Self::Storeroom => &STOREROOM_SEGMENTS,
            Self::Office => &OFFICE_SEGMENTS,
        }
    }

    #[must_use]
    pub const fn room_actors(self) -> &'static [HubRoomActor] {
        match self {
            Self::Courtyard => &[],
            Self::Mortuary => &MORTUARY_ACTORS,
            Self::Library => &LIBRARY_ACTORS,
            Self::Storeroom => &STOREROOM_ACTORS,
            Self::Office => &OFFICE_ACTORS,
        }
    }

    #[must_use]
    pub const fn room_props(self) -> &'static [HubRoomProp] {
        match self {
            Self::Courtyard => &[],
            Self::Mortuary => &MORTUARY_PROPS,
            Self::Library => &LIBRARY_PROPS,
            Self::Storeroom => &STOREROOM_PROPS,
            Self::Office => &OFFICE_PROPS,
        }
    }

    #[must_use]
    pub const fn flame_anchors(self) -> &'static [Vec2] {
        match self {
            Self::Courtyard => &[],
            Self::Mortuary => &MORTUARY_FLAMES,
            Self::Library => &LIBRARY_FLAMES,
            Self::Storeroom => &STOREROOM_FLAMES,
            Self::Office => &OFFICE_FLAMES,
        }
    }
}

/// The player is weaker than room fixtures (12 push strength against 90 resistance),
/// so `actor-physics.ts::resolveActorMotion` applies the full unweighted correction.
/// A correction must be a valid full placement; it does not sweep around a blocking wall.
#[must_use]
pub fn separate_from_room_actors(
    region: HubRegion,
    mut position: Vec2,
    bypass_segments: bool,
) -> Vec2 {
    let circles = region
        .room_actors()
        .iter()
        .map(|a| (a.position, a.radius))
        .chain(region.room_props().iter().map(|p| (p.position, p.radius)));
    for (center, radius) in circles {
        let dx = position.x - center.x;
        let dy = position.y - center.y;
        let distance_squared = dx * dx + dy * dy;
        let radius_sum = PLAYER_CHARACTER_RADIUS + radius;
        if distance_squared == 0.0 || distance_squared >= radius_sum * radius_sum {
            continue;
        }
        let distance = distance_squared.sqrt();
        let overlap = radius_sum + 0.1 - distance;
        let candidate =
            Vec2::new(position.x + dx / distance * overlap, position.y + dy / distance * overlap);
        if bypass_segments
            || is_traversable_against_segments(
                candidate,
                PLAYER_CHARACTER_RADIUS,
                region.segments(),
            )
        {
            position = candidate;
        }
    }
    position
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubRoomFlameTransform {
    pub rotation: f64,
    pub scale_x: f64,
    pub scale_y: f64,
}

/// `hubRoomFlameTransform`: a stateless hash keeps each flame's flicker independent.
#[must_use]
pub fn hub_room_flame_transform(
    region: HubRegion,
    tick: u64,
    index: usize,
) -> HubRoomFlameTransform {
    let (minimum, range) = if region == HubRegion::Mortuary { (0.7, 0.2) } else { (0.8, 0.4) };
    HubRoomFlameTransform {
        rotation: (-5.0 + unit_sample(region, tick, index, 1) * 10.0) * std::f64::consts::PI
            / 180.0,
        scale_x: 0.8,
        scale_y: minimum + unit_sample(region, tick, index, 0) * range,
    }
}

fn unit_sample(region: HubRegion, tick: u64, index: usize, lane: u32) -> f64 {
    let seed = match region {
        HubRegion::Mortuary => 0x02f6_e2b1,
        HubRegion::Storeroom => 0x04af_41d3,
        HubRegion::Library => 0x06c8_e9cf,
        HubRegion::Office => 0x08db_ca55,
        HubRegion::Courtyard => panic!("the courtyard has no room flames"),
    };
    let mut value = tick as u32
        ^ seed
        ^ (index as u32 + 1).wrapping_mul(0x9e37_79b1)
        ^ (lane + 1).wrapping_mul(0x85eb_ca6b);
    value = (value ^ value >> 16).wrapping_mul(0x7feb_352d);
    value = (value ^ value >> 15).wrapping_mul(0x846c_a68b);
    value ^= value >> 16;
    f64::from(value) / 4_294_967_296.0
}

/// `hubMemoratorHeadingIndex`: the sixteen-frame caretaker faces the local wizard.
#[must_use]
pub fn hub_memorator_heading_index(player_position: Vec2) -> usize {
    let radians = atan2(player_position.x - 628.0, -(player_position.y - 770.0));
    let heading = (radians + std::f64::consts::TAU) % std::f64::consts::TAU;
    (heading / std::f64::consts::TAU * 16.0).round() as usize % 16
}
