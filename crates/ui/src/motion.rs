//! The single motion language shared by every animation, matching the web
//! app's `motion-polish.css`: one curve — CSS
//! `cubic-bezier(0.2, 0.0, 0.0, 1.0)` — and three named durations. The
//! forked egui only ships preset easings and has no generic bezier, so the
//! curve is evaluated here by solving the x polynomial for the parametric
//! time and then reading the y polynomial.

/// Fast controls: toggles, the FAB entrance, hovers (web `.22s`).
pub const FAST: f64 = 0.22;
/// Standard navigation and page entrances (web `.32s`).
pub const BASE: f64 = 0.32;
/// Large surfaces and modal transitions (web `.36s`).
pub const SLOW: f64 = 0.36;

/// Pressed element scale (web `transform: scale(0.96)`).
pub const PRESS_SCALE: f32 = 0.96;

/// The animation time egui should use for a standard duration: the value
/// itself when motion is allowed, zero (instant settle) when the user asked
/// for reduced motion, mirroring the web reduced-motion media query.
#[must_use]
pub fn duration(seconds: f64, reduced_motion: bool) -> f32 {
    if reduced_motion { 0.0 } else { seconds as f32 }
}

const X1: f32 = 0.2;
const Y1: f32 = 0.0;
const X2: f32 = 0.0;
const Y2: f32 = 1.0;

fn bezier_x(s: f32) -> f32 {
    let r = 1.0 - s;
    3.0 * r * r * s * X1 + 3.0 * r * s * s * X2 + s * s * s
}

fn bezier_y(s: f32) -> f32 {
    let r = 1.0 - s;
    3.0 * r * r * s * Y1 + 3.0 * r * s * s * Y2 + s * s * s
}

fn bezier_dx(s: f32) -> f32 {
    let r = 1.0 - s;
    3.0 * r * r * X1 + 6.0 * r * s * (X2 - X1) + 3.0 * s * s * (1.0 - X2)
}

/// Evaluate `cubic-bezier(0.2, 0, 0, 1)` at linear time `t` in `[0, 1]`,
/// using Newton iteration on the x component (eight iterations land well
/// below one frame's visual precision for these control points).
#[must_use]
pub fn standard(t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    let mut s = t;
    for _ in 0..8 {
        let error = bezier_x(s) - t;
        if error.abs() < 1e-5 {
            break;
        }
        let derivative = bezier_dx(s);
        if derivative.abs() < 1e-6 {
            break;
        }
        s = (s - error / derivative).clamp(0.0, 1.0);
    }
    bezier_y(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_are_pinned() {
        assert_eq!(standard(0.0), 0.0);
        assert_eq!(standard(1.0), 1.0);
        assert_eq!(standard(-0.5), 0.0);
        assert_eq!(standard(1.5), 1.0);
    }

    #[test]
    fn curve_is_monotone_and_eases_out_fast() {
        let mut previous = 0.0_f32;
        for step in 1..=20 {
            let value = standard(step as f32 / 20.0);
            assert!(value >= previous, "curve must never decrease");
            previous = value;
        }
        // With (0.2, 0) / (0, 1), half the linear time already yields more
        // than half the progress: the characteristic quick settle.
        assert!(standard(0.5) > 0.5);
    }
}
