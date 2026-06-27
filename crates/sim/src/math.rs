//! Minimal 2D vector math for the continuous galaxy space.
//!
//! Uses `f64` throughout for deterministic, platform-stable arithmetic in the
//! pure core (no SIMD, no fast-math). Hand-rolled to keep the `sim` crate free
//! of external dependencies.

use std::ops::{Add, Div, Mul, Neg, Sub};

use serde::{Deserialize, Serialize};

/// A point or displacement in continuous 2D galaxy space (units: "su", sim units).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    pub const fn new(x: f64, y: f64) -> Self {
        Vec2 { x, y }
    }

    pub fn dot(self, o: Vec2) -> f64 {
        self.x * o.x + self.y * o.y
    }

    pub fn length_sq(self) -> f64 {
        self.dot(self)
    }

    pub fn length(self) -> f64 {
        self.length_sq().sqrt()
    }

    /// Euclidean distance to another point.
    pub fn distance(self, o: Vec2) -> f64 {
        (self - o).length()
    }

    /// Squared Euclidean distance (cheaper; no sqrt).
    pub fn distance_sq(self, o: Vec2) -> f64 {
        (self - o).length_sq()
    }

    /// Unit vector in the same direction, or ZERO if this is (near) zero-length.
    pub fn normalized(self) -> Vec2 {
        let len = self.length();
        if len <= f64::EPSILON {
            Vec2::ZERO
        } else {
            self / len
        }
    }

    /// Construct from a polar angle (radians) and radius.
    pub fn from_polar(angle: f64, radius: f64) -> Vec2 {
        Vec2::new(angle.cos() * radius, angle.sin() * radius)
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}

impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}

impl Mul<f64> for Vec2 {
    type Output = Vec2;
    fn mul(self, s: f64) -> Vec2 {
        Vec2::new(self.x * s, self.y * s)
    }
}

impl Div<f64> for Vec2 {
    type Output = Vec2;
    fn div(self, s: f64) -> Vec2 {
        Vec2::new(self.x / s, self.y / s)
    }
}
