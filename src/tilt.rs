//! Tilt input layer (docs/03-architecture.md).
//!
//! Produces a single tilt angle (radians) that gets fed into the sim. It is
//! isolated from both sim and render on purpose: adding tilt momentum/inertia,
//! a gamepad, or any other driver later touches *only* this layer — Phase 1
//! keeps it deliberately simple (instantaneous manual, slow auto see-saw).

use crate::sim::TAU;

// ----------------------------------------------------------------------------
// Tunables (docs/02-phase1-spec.md).
// ----------------------------------------------------------------------------

/// Auto see-saw period in seconds — slow, meant to be hypnotic, not frantic.
pub const AUTO_PERIOD: f32 = 7.0;

/// Auto see-saw peak tilt angle, radians (~7°).
pub const AUTO_MAX_ANGLE: f32 = 0.12;

/// Half-range of the manual slider, radians (~11°). Slider spans `±` this.
pub const MANUAL_MAX_ANGLE: f32 = 0.20;

/// Which driver currently owns the tilt.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TiltMode {
    /// Slow self-running sine oscillation (the powered see-saw). Default.
    Auto,
    /// Slider sets the angle directly and absolutely. Instantaneous.
    Manual,
}

/// Owns the current tilt and how it evolves.
pub struct TiltController {
    mode: TiltMode,
    /// The tilt fed to the sim this step (radians).
    angle: f32,
    /// The angle the slider is parked at, used while in `Manual`.
    manual_angle: f32,
    /// Elapsed sim-time, drives the auto oscillation.
    time: f32,
}

impl TiltController {
    /// Starts in Auto — set it down and watch (docs/01-overview.md).
    pub fn new() -> Self {
        Self {
            mode: TiltMode::Auto,
            angle: 0.0,
            manual_angle: 0.0,
            time: 0.0,
        }
    }

    /// Advance by `dt` seconds, updating the tilt according to the mode.
    pub fn advance(&mut self, dt: f32) {
        match self.mode {
            TiltMode::Auto => {
                self.time += dt;
                self.angle = AUTO_MAX_ANGLE * (TAU * self.time / AUTO_PERIOD).sin();
            }
            TiltMode::Manual => {
                self.angle = self.manual_angle;
            }
        }
    }

    /// The slider was touched: take over in Manual at the given angle.
    pub fn set_manual(&mut self, angle: f32) {
        self.mode = TiltMode::Manual;
        self.manual_angle = angle.clamp(-MANUAL_MAX_ANGLE, MANUAL_MAX_ANGLE);
        self.angle = self.manual_angle;
    }

    /// Hand control back to the auto see-saw, resuming its oscillation
    /// smoothly from the current angle so it doesn't jump.
    pub fn set_auto(&mut self) {
        self.mode = TiltMode::Auto;
        // Re-phase the oscillation so sin() starts at the current angle,
        // avoiding a visible snap when handing back.
        let ratio = (self.angle / AUTO_MAX_ANGLE).clamp(-1.0, 1.0);
        self.time = ratio.asin() * AUTO_PERIOD / TAU;
    }

    pub fn mode(&self) -> TiltMode {
        self.mode
    }

    /// Current tilt angle (radians) — read by the sim and by the renderer
    /// (the latter only for the visual see-saw rotation).
    pub fn angle(&self) -> f32 {
        self.angle
    }
}

impl Default for TiltController {
    fn default() -> Self {
        Self::new()
    }
}

// ----------------------------------------------------------------------------
// 2D tilt (Phase 2). A tilt *vector* drives the 2D sim. The auto behaviour is
// precession — the tilt direction slowly rotates so waves chase around the
// tank (docs/04-phase2-isometric.md). Kept as a separate, small controller so
// the Phase 1 path above stays untouched.
// ----------------------------------------------------------------------------

/// Seconds for the auto tilt direction to precess once around.
pub const PRECESS_PERIOD: f32 = 13.0;

/// Peak tilt magnitude on each axis in 2D mode, radians (~6°).
pub const TILT_2D_MAX_ANGLE: f32 = 0.10;

/// Owns the 2D tilt vector and how it evolves.
pub struct Tilt2dController {
    mode: TiltMode,
    x: f32,
    y: f32,
    manual_x: f32,
    manual_y: f32,
    time: f32,
}

impl Tilt2dController {
    /// Starts in Auto precession — set it down and watch.
    pub fn new() -> Self {
        Self {
            mode: TiltMode::Auto,
            x: TILT_2D_MAX_ANGLE,
            y: 0.0,
            manual_x: 0.0,
            manual_y: 0.0,
            time: 0.0,
        }
    }

    /// Advance by `dt` seconds, updating the tilt vector per the mode.
    pub fn advance(&mut self, dt: f32) {
        match self.mode {
            TiltMode::Auto => {
                // Constant-magnitude tilt whose direction rotates slowly.
                self.time += dt;
                let a = TAU * self.time / PRECESS_PERIOD;
                self.x = TILT_2D_MAX_ANGLE * a.cos();
                self.y = TILT_2D_MAX_ANGLE * a.sin();
            }
            TiltMode::Manual => {
                self.x = self.manual_x;
                self.y = self.manual_y;
            }
        }
    }

    /// The tilt pad was touched: take over in Manual at the given vector.
    pub fn set_manual(&mut self, x: f32, y: f32) {
        self.mode = TiltMode::Manual;
        self.manual_x = x.clamp(-TILT_2D_MAX_ANGLE, TILT_2D_MAX_ANGLE);
        self.manual_y = y.clamp(-TILT_2D_MAX_ANGLE, TILT_2D_MAX_ANGLE);
        self.x = self.manual_x;
        self.y = self.manual_y;
    }

    /// Hand control back to auto precession, re-phasing from the current
    /// direction so the tilt doesn't snap.
    pub fn set_auto(&mut self) {
        self.mode = TiltMode::Auto;
        let a = self.y.atan2(self.x);
        self.time = a * PRECESS_PERIOD / TAU;
    }

    pub fn mode(&self) -> TiltMode {
        self.mode
    }
    pub fn tilt_x(&self) -> f32 {
        self.x
    }
    pub fn tilt_y(&self) -> f32 {
        self.y
    }
}

impl Default for Tilt2dController {
    fn default() -> Self {
        Self::new()
    }
}
