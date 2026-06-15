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
