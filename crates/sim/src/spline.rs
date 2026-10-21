//! Native natural cubic spline compilation/evaluation from `native-natural-spline.ts`.
//! The axis arithmetic was first ported for Hub students and is shared with Spider Silk.
use crate::Vec2;

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NaturalSplineAxis {
    pub points: Vec<f64>,
    pub coefficients: Vec<[f64; 3]>,
}
impl NaturalSplineAxis {
    #[must_use]
    pub fn new(points: Vec<f64>) -> Self {
        assert!(points.len() >= 2, "a native spline needs at least two points");
        let segments = points.len() - 1;
        let mut ratio = vec![0.0; points.len()];
        let mut solution = vec![0.0; points.len()];
        for index in 1..segments {
            let alpha = 3.0 * (points[index + 1] - points[index])
                - 3.0 * (points[index] - points[index - 1]);
            let lower = 4.0 - ratio[index - 1];
            ratio[index] = 1.0 / lower;
            solution[index] = (alpha - solution[index - 1]) / lower;
        }
        let mut curvature = vec![0.0; points.len()];
        let mut coefficients = vec![[0.0; 3]; segments];
        for index in (0..segments).rev() {
            curvature[index] = solution[index] - ratio[index] * curvature[index + 1];
            let linear = points[index + 1]
                - points[index]
                - (curvature[index + 1] + 2.0 * curvature[index]) / 3.0;
            coefficients[index] =
                [linear, curvature[index], (curvature[index + 1] - curvature[index]) / 3.0];
        }
        Self { points, coefficients }
    }
    #[must_use]
    pub fn evaluate(&self, cursor: f64) -> f64 {
        if cursor <= 0.0 {
            return self.points[0];
        }
        if cursor >= (self.points.len() - 1) as f64 {
            return self.points[self.points.len() - 1];
        }
        let segment = cursor.floor() as usize;
        let progress = cursor - segment as f64;
        let [linear, quadratic, cubic] = self.coefficients[segment];
        self.points[segment] + progress * (linear + progress * (quadratic + progress * cubic))
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NaturalSpline {
    pub extent: f64,
    pub x: NaturalSplineAxis,
    pub y: NaturalSplineAxis,
}
impl NaturalSpline {
    #[must_use]
    pub fn new(points: &[Vec2]) -> Self {
        assert!(points.len() >= 2, "a native spline needs at least two points");
        Self {
            extent: (points.len() - 1) as f64,
            x: NaturalSplineAxis::new(points.iter().map(|point| point.x).collect()),
            y: NaturalSplineAxis::new(points.iter().map(|point| point.y).collect()),
        }
    }
    #[must_use]
    pub fn evaluate(&self, cursor: f64) -> Vec2 {
        Vec2::new(self.x.evaluate(cursor), self.y.evaluate(cursor))
    }
}
