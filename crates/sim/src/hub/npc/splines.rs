//! The eighteen stock Courtyard QuickSpline routes, from Website
//! `core-server/hub-student-splines.ts` and `native-natural-spline.ts`.

use crate::Vec2;
use crate::spline::NaturalSpline;

/// Control points in the stock path-id order.
pub const HUB_STUDENT_SPLINES: [&[Vec2]; 18] = [
    &[
        Vec2::new(1577.0, -29.0),
        Vec2::new(1550.0, 131.0),
        Vec2::new(1439.0, 298.0),
        Vec2::new(1212.0, 524.0),
        Vec2::new(938.0, 568.0),
        Vec2::new(787.0, 497.0),
        Vec2::new(751.0, 386.0),
        Vec2::new(767.0, 296.0),
        Vec2::new(803.0, 201.0),
        Vec2::new(773.0, 135.0),
        Vec2::new(716.0, 77.0),
        Vec2::new(663.0, 72.0),
        Vec2::new(627.0, 74.0),
        Vec2::new(456.0, 77.0),
    ],
    &[
        Vec2::new(1594.0, -33.0),
        Vec2::new(1568.0, 140.0),
        Vec2::new(1489.0, 253.0),
        Vec2::new(1368.0, 416.0),
        Vec2::new(1216.0, 617.0),
        Vec2::new(1105.0, 842.0),
        Vec2::new(977.0, 954.0),
        Vec2::new(934.0, 1123.0),
    ],
    &[
        Vec2::new(65.0, 336.0),
        Vec2::new(167.0, 498.0),
        Vec2::new(328.0, 654.0),
        Vec2::new(378.0, 710.0),
        Vec2::new(424.0, 831.0),
        Vec2::new(521.0, 874.0),
        Vec2::new(678.0, 873.0),
        Vec2::new(934.0, 920.0),
        Vec2::new(1225.0, 930.0),
        Vec2::new(1459.0, 926.0),
        Vec2::new(1656.0, 956.0),
        Vec2::new(1749.0, 1078.0),
    ],
    &[
        Vec2::new(989.0, 1140.0),
        Vec2::new(1003.0, 956.0),
        Vec2::new(1177.0, 845.0),
        Vec2::new(1471.0, 783.0),
        Vec2::new(1713.0, 710.0),
        Vec2::new(1952.0, 576.0),
        Vec2::new(2048.0, 495.0),
    ],
    &[
        Vec2::new(16.0, 366.0),
        Vec2::new(90.0, 511.0),
        Vec2::new(69.0, 652.0),
        Vec2::new(-54.0, 757.0),
    ],
    &[
        Vec2::new(1644.0, -31.0),
        Vec2::new(1560.0, 292.0),
        Vec2::new(1572.0, 502.0),
        Vec2::new(1639.0, 621.0),
        Vec2::new(1734.0, 666.0),
        Vec2::new(1874.0, 622.0),
        Vec2::new(2053.0, 484.0),
    ],
    &[
        Vec2::new(1998.0, 453.0),
        Vec2::new(1841.0, 567.0),
        Vec2::new(1717.0, 618.0),
        Vec2::new(1540.0, 580.0),
        Vec2::new(1280.0, 568.0),
        Vec2::new(1148.0, 604.0),
        Vec2::new(888.0, 623.0),
        Vec2::new(627.0, 600.0),
        Vec2::new(349.0, 580.0),
        Vec2::new(166.0, 509.0),
        Vec2::new(48.0, 333.0),
    ],
    &[
        Vec2::new(-53.0, 814.0),
        Vec2::new(239.0, 782.0),
        Vec2::new(367.0, 741.0),
        Vec2::new(401.0, 668.0),
        Vec2::new(477.0, 620.0),
        Vec2::new(638.0, 634.0),
        Vec2::new(884.0, 669.0),
        Vec2::new(1073.0, 672.0),
        Vec2::new(1260.0, 621.0),
        Vec2::new(1412.0, 442.0),
        Vec2::new(1530.0, 268.0),
        Vec2::new(1695.0, -33.0),
    ],
    &[
        Vec2::new(2031.0, 929.0),
        Vec2::new(1462.0, 888.0),
        Vec2::new(1221.0, 892.0),
        Vec2::new(987.0, 904.0),
        Vec2::new(884.0, 978.0),
        Vec2::new(873.0, 1116.0),
    ],
    &[
        Vec2::new(895.0, 1121.0),
        Vec2::new(841.0, 987.0),
        Vec2::new(549.0, 980.0),
        Vec2::new(189.0, 977.0),
        Vec2::new(-42.0, 969.0),
    ],
    &[
        Vec2::new(2044.0, 109.0),
        Vec2::new(1833.0, 137.0),
        Vec2::new(1634.0, 232.0),
        Vec2::new(1536.0, 390.0),
        Vec2::new(1541.0, 547.0),
        Vec2::new(1626.0, 653.0),
        Vec2::new(1797.0, 653.0),
        Vec2::new(2064.0, 487.0),
    ],
    &[
        Vec2::new(848.0, 1133.0),
        Vec2::new(859.0, 799.0),
        Vec2::new(821.0, 574.0),
        Vec2::new(760.0, 415.0),
        Vec2::new(780.0, 227.0),
        Vec2::new(778.0, 151.0),
        Vec2::new(733.0, 95.0),
        Vec2::new(672.0, 71.0),
        Vec2::new(608.0, 75.0),
        Vec2::new(473.0, 77.0),
    ],
    &[
        Vec2::new(1477.0, -49.0),
        Vec2::new(1453.0, 3.0),
        Vec2::new(1410.0, 46.0),
        Vec2::new(1360.0, 59.0),
        Vec2::new(1327.0, 96.0),
        Vec2::new(1350.0, 144.0),
        Vec2::new(1421.0, 224.0),
        Vec2::new(1412.0, 352.0),
        Vec2::new(1369.0, 453.0),
        Vec2::new(1315.0, 510.0),
        Vec2::new(1231.0, 561.0),
        Vec2::new(1154.0, 535.0),
        Vec2::new(1177.0, 448.0),
        Vec2::new(1193.0, 385.0),
        Vec2::new(1183.0, 307.0),
        Vec2::new(1157.0, 241.0),
        Vec2::new(1101.0, 183.0),
        Vec2::new(1026.0, 185.0),
        Vec2::new(974.0, 100.0),
        Vec2::new(973.0, -44.0),
    ],
    &[
        Vec2::new(918.0, 1149.0),
        Vec2::new(826.0, 950.0),
        Vec2::new(719.0, 737.0),
        Vec2::new(558.0, 599.0),
        Vec2::new(389.0, 604.0),
        Vec2::new(241.0, 566.0),
        Vec2::new(137.0, 470.0),
        Vec2::new(23.0, 275.0),
    ],
    &[
        Vec2::new(2031.0, 429.0),
        Vec2::new(1836.0, 576.0),
        Vec2::new(1614.0, 636.0),
        Vec2::new(1466.0, 589.0),
        Vec2::new(1285.0, 590.0),
        Vec2::new(1048.0, 668.0),
        Vec2::new(771.0, 649.0),
        Vec2::new(542.0, 599.0),
        Vec2::new(371.0, 658.0),
        Vec2::new(155.0, 736.0),
        Vec2::new(-49.0, 778.0),
    ],
    &[
        Vec2::new(1474.0, -49.0),
        Vec2::new(1451.0, 3.0),
        Vec2::new(1412.0, 44.0),
        Vec2::new(1361.0, 58.0),
        Vec2::new(1329.0, 96.0),
        Vec2::new(1352.0, 143.0),
        Vec2::new(1427.0, 235.0),
        Vec2::new(1474.0, 415.0),
        Vec2::new(1508.0, 594.0),
        Vec2::new(1560.0, 745.0),
        Vec2::new(1669.0, 901.0),
        Vec2::new(1799.0, 1075.0),
    ],
    &[
        Vec2::new(-35.0, 997.0),
        Vec2::new(148.0, 868.0),
        Vec2::new(217.0, 651.0),
        Vec2::new(126.0, 441.0),
        Vec2::new(29.0, 293.0),
    ],
    &[
        Vec2::new(1850.0, 1073.0),
        Vec2::new(1703.0, 887.0),
        Vec2::new(1602.0, 733.0),
        Vec2::new(1566.0, 626.0),
        Vec2::new(1599.0, 501.0),
        Vec2::new(1592.0, 331.0),
        Vec2::new(1625.0, 191.0),
        Vec2::new(1753.0, 80.0),
        Vec2::new(1878.0, 25.0),
        Vec2::new(1971.0, 16.0),
    ],
];

/// Compiled natural splines; the population compiles its immutable routes once.
#[derive(Clone, Debug)]
pub struct HubStudentSplines {
    splines: Vec<NaturalSpline>,
}

impl Default for HubStudentSplines {
    fn default() -> Self {
        Self::new()
    }
}

impl HubStudentSplines {
    #[must_use]
    pub fn new() -> Self {
        let mut splines = Vec::with_capacity(HUB_STUDENT_SPLINES.len());
        for points in HUB_STUDENT_SPLINES {
            splines.push(NaturalSpline::new(points));
        }
        Self { splines }
    }

    /// `evaluateHubStudentSpline`, clamped at both route endpoints.
    #[must_use]
    pub fn evaluate(&self, path_id: usize, cursor: f64) -> Vec2 {
        self.splines[path_id].evaluate(cursor)
    }
}
