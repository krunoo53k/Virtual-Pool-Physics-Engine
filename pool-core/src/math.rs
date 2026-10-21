//! Minimal, allocation-free linear algebra for the deterministic physics core.
//!
//! Every quantity is `f64` (§10.4, §11.1): the core never mixes numeric types and
//! never widens to `f32`. The helpers here are deliberately explicit about the
//! degenerate cases (zero-length normals, non-finite inputs) so that the physics
//! code can never produce a `NaN` by accident.

use core::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

/// A three dimensional vector.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub const X: Vec3 = Vec3 {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    };
    pub const Y: Vec3 = Vec3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };
    pub const Z: Vec3 = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    };

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Vec3 { x, y, z }
    }

    pub const fn splat(v: f64) -> Self {
        Vec3 { x: v, y: v, z: v }
    }

    pub fn dot(self, other: Vec3) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(self, other: Vec3) -> Vec3 {
        Vec3 {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }

    pub fn norm_squared(self) -> f64 {
        self.dot(self)
    }

    pub fn norm(self) -> f64 {
        self.norm_squared().sqrt()
    }

    /// Normalises the vector, returning `None` for a zero-length or non-finite input.
    ///
    /// The caller is expected to handle the `None` case explicitly (ground rule 6).
    pub fn normalize(self) -> Option<Vec3> {
        let n = self.norm();
        if !n.is_finite() || n <= 0.0 {
            return None;
        }
        Some(self / n)
    }

    /// True when every component is finite (guards against `NaN` poisoning, §11.4).
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    pub fn get(self, i: usize) -> f64 {
        match i {
            0 => self.x,
            1 => self.y,
            _ => self.z,
        }
    }

    pub fn set(&mut self, i: usize, v: f64) {
        match i {
            0 => self.x = v,
            1 => self.y = v,
            _ => self.z = v,
        }
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl AddAssign for Vec3 {
    fn add_assign(&mut self, o: Vec3) {
        self.x += o.x;
        self.y += o.y;
        self.z += o.z;
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f64) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}

impl Mul<Vec3> for f64 {
    type Output = Vec3;
    fn mul(self, v: Vec3) -> Vec3 {
        v * self
    }
}

impl Div<f64> for Vec3 {
    type Output = Vec3;
    fn div(self, s: f64) -> Vec3 {
        Vec3::new(self.x / s, self.y / s, self.z / s)
    }
}

impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}

/// A two dimensional vector, used for the table plan geometry (rails, pockets).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };
    pub const X: Vec2 = Vec2 { x: 1.0, y: 0.0 };
    pub const Y: Vec2 = Vec2 { x: 0.0, y: 1.0 };

    pub const fn new(x: f64, y: f64) -> Self {
        Vec2 { x, y }
    }

    pub fn dot(self, other: Vec2) -> f64 {
        self.x * other.x + self.y * other.y
    }

    /// The scalar (z) component of the 3D cross product.
    pub fn cross(self, other: Vec2) -> f64 {
        self.x * other.y - self.y * other.x
    }

    pub fn norm_squared(self) -> f64 {
        self.dot(self)
    }

    pub fn norm(self) -> f64 {
        self.norm_squared().sqrt()
    }

    pub fn normalize(self) -> Option<Vec2> {
        let n = self.norm();
        if !n.is_finite() || n <= 0.0 {
            return None;
        }
        Some(self / n)
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    /// Rotates the vector counter-clockwise by `angle` radians.
    ///
    /// Uses the `libm` crate rather than `f64::sin`/`f64::cos` so that the result is
    /// identical on every platform (§11.4 risk 1).
    pub fn rotated(self, angle: f64) -> Vec2 {
        let (s, c) = (libm::sin(angle), libm::cos(angle));
        Vec2::new(self.x * c - self.y * s, self.x * s + self.y * c)
    }

    /// The angle of the vector in radians, in `(-pi, pi]`.
    pub fn angle(self) -> f64 {
        libm::atan2(self.y, self.x)
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}

impl AddAssign for Vec2 {
    fn add_assign(&mut self, o: Vec2) {
        self.x += o.x;
        self.y += o.y;
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
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

impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}

/// Compares two `f64` values with a *total* order (`NaN` included), so that event
/// sorting never depends on `partial_cmp` returning `None` (§11.4 risk 4).
pub fn total_cmp(a: f64, b: f64) -> core::cmp::Ordering {
    a.total_cmp(&b)
}

/// The smallest impact time that is treated as a real event, s.
///
/// The positions of the balls are known to about a picometre, so a contact that is
/// reported a fraction of a nanosecond away is the rounding noise of the contact
/// that has just been resolved, not a new one. Every real contact lasts far longer
/// than this: the ball-ball contact alone is 250-300 microseconds.
pub const MIN_IMPACT_TIME: f64 = 1e-9;

/// Returns `true` when the value is a usable, finite number.
pub fn is_finite(v: f64) -> bool {
    v.is_finite()
}

/// `sqrt` guarded against negative arguments (ground rule 6).
pub fn safe_sqrt(v: f64) -> f64 {
    if v <= 0.0 {
        0.0
    } else {
        v.sqrt()
    }
}

/// Division guarded against a zero denominator (ground rule 6).
pub fn safe_div(num: f64, den: f64) -> f64 {
    if den == 0.0 || !den.is_finite() {
        0.0
    } else {
        num / den
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_rejects_zero_length() {
        assert!(Vec3::ZERO.normalize().is_none());
        assert!(Vec2::ZERO.normalize().is_none());
    }

    #[test]
    fn cross_products_are_consistent() {
        let x = Vec3::X;
        let y = Vec3::Y;
        assert_eq!(x.cross(y), Vec3::Z);
        assert_eq!(y.cross(x), Vec3::Z.neg());
    }

    #[test]
    fn rotation_by_90_degrees() {
        let r = Vec2::X.rotated(core::f64::consts::FRAC_PI_2);
        assert!((r.x - 0.0).abs() < 1e-12);
        assert!((r.y - 1.0).abs() < 1e-12);
    }
}
