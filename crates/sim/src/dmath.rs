//! Deterministic transcendental functions for the battle engine (rules v16
//! and later; see `tactical::Math`).
//!
//! The standard library's `sin`, `cos`, `atan2` and `powf` call the
//! platform's libm, and the optimizer may rewrite those calls. In an
//! optimized build on Apple platforms LLVM fuses a `sin` and a `cos` of the
//! same angle into one `__sincos_stret` call, and turns `powf(x, 0.5)` with a
//! constant exponent into `sqrt(x)`. Each differs from the plain call in the
//! last bit for about one input in seven hundred, so a debug build and a
//! release build fought the same battle differently, and an OS update,
//! another platform or an unrelated code change (which moves the
//! optimizer's choices) could do the same to an archived replay.
//!
//! These are software (the `libm` crate, a port of musl's libm): every build
//! on every platform computes the same bits. Arithmetic and `sqrt` are
//! exactly rounded by IEEE 754 and need nothing here.

use crate::math::Vec2;

pub fn sin(x: f64) -> f64 {
    libm::sin(x)
}

pub fn cos(x: f64) -> f64 {
    libm::cos(x)
}

pub fn sin_cos(x: f64) -> (f64, f64) {
    (libm::sin(x), libm::cos(x))
}

pub fn atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}

pub fn powf(x: f64, e: f64) -> f64 {
    libm::pow(x, e)
}

/// `Vec2::from_polar`, in software.
pub fn polar(angle: f64, radius: f64) -> Vec2 {
    let (s, c) = sin_cos(angle);
    Vec2::new(c * radius, s * radius)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pinned bits over a spread of the inputs battles use (angles of either
    /// sign past a full turn, ratios, exponents). The same in debug and
    /// release, on any platform: a change here means the battle engine's
    /// math is no longer the software it claims to be.
    #[test]
    fn the_same_bits_in_every_build() {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut mix = |v: f64| h = (h ^ v.to_bits()).wrapping_mul(0x100000001b3);
        for i in 0..20_000 {
            let x = (i as f64 - 10_000.0) * 0.001_37;
            let (s, c) = sin_cos(x);
            assert_eq!((s, c), (sin(x), cos(x)), "one sin and cos, paired or not");
            mix(s);
            mix(c);
            mix(atan2(x, 0.73));
            mix(powf(x.abs() + 0.01, 0.5));
            mix(powf(x.abs() + 0.01, 0.35));
            let p = polar(x, 3.0);
            mix(p.x);
            mix(p.y);
        }
        assert_eq!(h, PINNED, "got {h:#018x}");
    }

    const PINNED: u64 = 0xd08c_cd51_e015_a4e1;
}
