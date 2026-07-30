#![allow(dead_code)]
use bevy_math::{Quat, Vec3};

pub fn normalized_clamp(value: f32, min: f32, max: f32) -> f32 {
    assert!(max > min, "max must be greater than min");
    let val = value.clamp(min, max);
    (val - min) / (max - min)
}

pub fn sign_side(n: f32) -> i32 {
    if n >= 0.0 { 1 } else { -1 }
}

pub fn curve(source: f32, bias: f32) -> f32 {
    let pi = std::f32::consts::PI;
    (((source - 0.5) * pi).sin() * 0.5 + 0.5) * bias + source * (1.0 - bias)
}

/// Port of Blunted2's `Vector3::GetNormalized(fallback)`: normalizes, or returns
/// the fallback direction when the vector is (near) zero-length.
pub fn normalized_or(v: Vec3, fallback: Vec3) -> Vec3 {
    if v.length_squared() < 1e-12 {
        fallback
    } else {
        v.normalize()
    }
}

/// Small deterministic PRNG (xorshift32), stand-in for the engine's `random(min, max)`.
#[derive(Debug, Clone)]
pub struct XorShift32(pub u32);

impl XorShift32 {
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// Uniform float in [min, max), like the C++ `random(min, max)`.
    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        let unit = (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32;
        min + unit * (max - min)
    }
}

use bevy_math::Vec2;

/// Port of Blunted2's `Line::GetDistanceToPoint(point, &u)` in 2D: distance from
/// `p` to the infinite line through v0-v1, plus the unclamped parameter `u`
/// (0 at v0, 1 at v1). Callers use `u` to know where along the segment the
/// closest approach happens.
pub fn line_distance_to_point_2d(v0: Vec2, v1: Vec2, p: Vec2) -> (f32, f32) {
    let d = v1 - v0;
    let len2 = d.length_squared();
    if len2 < 1e-12 {
        return ((p - v0).length(), 0.0);
    }
    let u = (p - v0).dot(d) / len2;
    let closest = v0 + d * u;
    ((p - closest).length(), u)
}

/// Port of `Line::GetIntersectionPoint` in 2D (infinite lines). Returns the
/// intersection point and the parameter `u` along the first line (0 at a0,
/// 1 at a1). Parallel lines return the midpoint of a with u = 0.
pub fn line_intersection_2d(a0: Vec2, a1: Vec2, b0: Vec2, b1: Vec2) -> (Vec2, f32) {
    let r = a1 - a0;
    let s = b1 - b0;
    let denom = r.x * s.y - r.y * s.x;
    if denom.abs() < 1e-9 {
        return (a0, 0.0);
    }
    let q = b0 - a0;
    let u = (q.x * s.y - q.y * s.x) / denom;
    (a0 + r * u, u)
}

/// Which side of the (v0 → v1) line `p` lies on (port of `Line::WhatSide`).
pub fn what_side_2d(v0: Vec2, v1: Vec2, p: Vec2) -> bool {
    let d = v1 - v0;
    let q = p - v0;
    d.x * q.y - d.y * q.x > 0.0
}

/// 2D rotation (port of `Vector3::GetRotated2D`).
pub fn rotated_2d(v: Vec2, angle: f32) -> Vec2 {
    let (s, c) = angle.sin_cos();
    Vec2::new(v.x * c - v.y * s, v.x * s + v.y * c)
}

/// `Vector3::GetNormalized(fallback)` for Vec2.
pub fn normalized_or_2d(v: Vec2, fallback: Vec2) -> Vec2 {
    if v.length_squared() < 1e-12 {
        fallback
    } else {
        v.normalize()
    }
}

pub fn modulate_into_range(min: f32, max: f32, mut value: f32) -> f32 {
    let step = max - min;
    while value < min {
        value += step;
    }
    while value > max {
        value -= step;
    }
    value
}

pub trait Vec3Ext {
    fn get_angle_2d(&self) -> f32;
    fn get_angle_2d_between(&self, test: Vec3) -> f32;
    fn get_2d(&self) -> Vec3;
    fn enforce_maximum_deviation(&self, deviant: Vec3, max_deviation: f32) -> Vec3;
    fn get_clamped_2d(&self, v1: Vec3, v2: Vec3) -> Vec3;
    fn get_normalized_max(&self, max_length: f32) -> Vec3;
    fn get_normalized_to(&self, length: f32) -> Vec3;
}

impl Vec3Ext for Vec3 {
    fn get_angle_2d(&self) -> f32 {
        let mut angle = self.y.atan2(self.x);
        if angle < 0.0 {
            angle += 2.0 * std::f32::consts::PI;
        }
        angle
    }

    fn get_angle_2d_between(&self, test: Vec3) -> f32 {
        let angle = -((self.x * test.y - self.y * test.x).atan2(self.x * test.x + self.y * test.y));
        modulate_into_range(-std::f32::consts::PI, std::f32::consts::PI, angle)
    }

    fn get_2d(&self) -> Vec3 {
        Vec3::new(self.x, self.y, 0.0)
    }

    fn enforce_maximum_deviation(&self, deviant: Vec3, max_deviation: f32) -> Vec3 {
        let mut result = *self;
        let difference = deviant - *self;
        let difference_distance = difference.length();
        if difference_distance > max_deviation {
            let normalized_diff = difference.normalize_or_zero();
            result += normalized_diff * (difference_distance - max_deviation);
        }
        result
    }

    fn get_clamped_2d(&self, v1: Vec3, v2: Vec3) -> Vec3 {
        let mut result = *self;
        let v1_to_v2 = v2.get_angle_2d_between(v1);
        let direction = sign_side(v1_to_v2);
        let v1_to_this = self.get_angle_2d_between(v1);
        let v2_to_this = self.get_angle_2d_between(v2);
        if sign_side(v1_to_this) != direction || sign_side(v2_to_this) == direction {
            if v1_to_this.abs() < v2_to_this.abs() {
                result = v1;
            } else {
                result = v2;
            }
        }
        result
    }

    fn get_normalized_max(&self, max_length: f32) -> Vec3 {
        if self.length() > max_length {
            self.normalize_or_zero() * max_length
        } else {
            *self
        }
    }

    fn get_normalized_to(&self, length: f32) -> Vec3 {
        self.normalize_or_zero() * length
    }
}

pub trait QuatExt {
    fn get_angles(&self) -> (f32, f32, f32);
    fn from_angles(x: f32, y: f32, z: f32) -> Quat;
    fn get_rotation_to(&self, to: Quat) -> Quat;
    fn get_rotation_multiplied_by(&self, factor: f32) -> Quat;
    fn get_rotation_angle(&self, to: Quat) -> f32;
}

impl QuatExt for Quat {
    fn get_angles(&self) -> (f32, f32, f32) {
        let (ex, ey, ez, ew) = (self.x, self.y, self.z, self.w);
        let singularity_test = ex * ez + ey * ew;
        if singularity_test > 0.49999 {
            let z_angle = 2.0 * ex.atan2(ey);
            let y_angle = std::f32::consts::PI * 0.5;
            let x_angle = 0.0;
            return (x_angle, y_angle, z_angle);
        }
        if singularity_test < -0.49999 {
            let z_angle = -2.0 * ex.atan2(ey);
            let y_angle = -std::f32::consts::PI * 0.5;
            let x_angle = 0.0;
            return (x_angle, y_angle, z_angle);
        }
        let sqx = ex * ex;
        let sqy = ez * ez;
        let sqz = ey * ey;

        let z_angle = (2.0 * ez * ew - 2.0 * ex * ey).atan2(1.0 - 2.0 * sqy - 2.0 * sqz);
        let y_angle = (2.0 * ex * ez + 2.0 * ey * ew).asin();
        let x_angle = (2.0 * ex * ew - 2.0 * ez * ey).atan2(1.0 - 2.0 * sqx - 2.0 * sqz);
        (x_angle, y_angle, z_angle)
    }

    fn from_angles(x: f32, y: f32, z: f32) -> Quat {
        let c1 = (y / 2.0).cos();
        let s1 = (y / 2.0).sin();
        let c2 = (z / 2.0).cos();
        let s2 = (z / 2.0).sin();
        let c3 = (x / 2.0).cos();
        let s3 = (x / 2.0).sin();
        let c1c2 = c1 * c2;
        let s1s2 = s1 * s2;
        let w = c1c2 * c3 - s1s2 * s3;
        let qx = c1c2 * s3 + s1s2 * c3;
        let qy = s1 * c2 * c3 + c1 * s2 * s3;
        let qz = c1 * s2 * c3 - s1 * c2 * s3;
        Quat::from_xyzw(qx, qy, qz, w)
    }

    fn get_rotation_to(&self, to: Quat) -> Quat {
        to * self.inverse()
    }

    fn get_rotation_multiplied_by(&self, factor: f32) -> Quat {
        let pi = std::f32::consts::PI;
        let (axis, mut angle) = self.to_axis_angle();
        if angle > pi {
            angle -= 2.0 * pi; // range -pi .. pi
        }
        angle = (angle * factor) % (2.0 * pi); // remove multiples of 2pi
        Quat::from_axis_angle(axis, angle).normalize()
    }

    fn get_rotation_angle(&self, to: Quat) -> f32 {
        let dot = self.dot(to).clamp(-1.0, 1.0);
        2.0 * dot.acos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `GetRotationMultipliedBy` must scale the SHORT way around: for a
    /// quaternion in the negative neighborhood (w < 0, axis-angle > π), scaling
    /// by 0.5 has to halve the short-path angle, not the long-path one.
    #[test]
    fn test_rotation_multiplied_by_negative_neighborhood() {
        let angle = 0.4f32;
        let q = Quat::from_axis_angle(Vec3::Z, angle);
        let q_neg = -q; // same rotation, negative neighborhood (w < 0)

        let half = q.get_rotation_multiplied_by(0.5);
        let half_neg = q_neg.get_rotation_multiplied_by(0.5);

        let expected = Quat::from_axis_angle(Vec3::Z, angle * 0.5);
        assert!(half.angle_between(expected) < 1e-4);
        assert!(
            half_neg.angle_between(expected) < 1e-4,
            "negative-neighborhood quat scaled the long way: {half_neg:?} vs {expected:?}"
        );
    }
}
