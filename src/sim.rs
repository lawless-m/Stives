//! 1D shallow-water simulation (side-on).
//!
//! ARCHITECTURE RULE (docs/03-architecture.md): this module knows *nothing*
//! about how it is drawn. No Macroquad, no pixels, no colours, no screen
//! coordinates, no side-on-vs-isometric. It owns the grid of state, steps the
//! physics given a tilt, and exposes the state read-only. Keeping this boundary
//! clean is what makes the Phase 2 isometric upgrade additive rather than a
//! rewrite.

use std::f32::consts::PI;

// ----------------------------------------------------------------------------
// Tunables (docs/02-phase1-spec.md). All deliberately easy to find and tweak.
// ----------------------------------------------------------------------------

/// Number of water columns across the tank width (tank resolution).
pub const NUM_CELLS: usize = 160;

/// Physical tank width in sim units. Cell spacing `DX` derives from this.
pub const TANK_WIDTH: f32 = 4.0;

/// Mean / rest water depth in sim units. Volume is conserved around this.
pub const REST_HEIGHT: f32 = 1.0;

/// Gravity strength. Drives both the restoring (pressure) force from height
/// differences and the tilt-induced horizontal slosh.
pub const GRAVITY: f32 = 5.0;

/// Per-substep velocity damping multiplier (`< 1.0`). Lets the water settle
/// between and within tilts, matching the original's calm-between-tips feel.
/// `1.0` would be no damping.
pub const DAMPING: f32 = 0.9997;

/// Fixed physics timestep, in seconds (the sim is stepped on an accumulator).
/// Chosen well inside the CFL stability limit for the constants above.
pub const DT: f32 = 0.004;

/// Distance between adjacent columns, derived from the tank width.
pub const DX: f32 = TANK_WIDTH / NUM_CELLS as f32;

/// Minimum column height — a small floor that keeps columns positive and the
/// scheme stable even under an aggressive tilt. With the default constants the
/// water never drains this far, so volume stays effectively conserved.
const MIN_HEIGHT: f32 = 0.02;

/// The shallow-water state: a flat array of water columns and the horizontal
/// velocities on the faces between them.
///
/// State layout:
/// - `heights[i]` — water column height of cell `i` (len `NUM_CELLS`).
/// - `vel[i]` — horizontal velocity on the face left of cell `i`
///   (len `NUM_CELLS + 1`; `vel[0]` and `vel[NUM_CELLS]` are the reflecting
///   walls and stay `0.0`).
pub struct WaterSim {
    heights: Vec<f32>,
    vel: Vec<f32>,
    /// Scratch buffer for per-face flux, kept around to avoid per-frame allocs.
    flux: Vec<f32>,
}

impl WaterSim {
    /// A calm, flat, full tank.
    pub fn new() -> Self {
        Self {
            heights: vec![REST_HEIGHT; NUM_CELLS],
            vel: vec![0.0; NUM_CELLS + 1],
            flux: vec![0.0; NUM_CELLS + 1],
        }
    }

    /// Flatten the water and kill all motion (the "calm" reset).
    pub fn reset(&mut self) {
        for h in &mut self.heights {
            *h = REST_HEIGHT;
        }
        for v in &mut self.vel {
            *v = 0.0;
        }
    }

    /// Advance the physics by `dt` seconds under the given `tilt` (radians).
    ///
    /// A positive tilt tips the `+x` (right) end of the tank down, so gravity
    /// gains a horizontal component pushing water toward the right; waves
    /// reflect off the two walls and the volume is conserved.
    pub fn step(&mut self, dt: f32, tilt: f32) {
        // Horizontal component of gravity from the tilt — the slosh driver.
        let g_horizontal = GRAVITY * tilt.sin();

        // 1. Update velocities on the interior faces. Walls (face 0 and
        //    NUM_CELLS) are reflecting and remain exactly 0.0.
        for i in 1..NUM_CELLS {
            let height_gradient = (self.heights[i] - self.heights[i - 1]) / DX;
            // Pressure (restoring) force pushes from tall columns to short ones;
            // the tilt term drives the bulk slosh.
            self.vel[i] += dt * (-GRAVITY * height_gradient + g_horizontal);
            self.vel[i] *= DAMPING;
        }

        // 2. Compute upwind flux on each face. Ends stay 0 (no flow through
        //    walls => water is conserved, since interior fluxes telescope).
        for i in 1..NUM_CELLS {
            let u = self.vel[i];
            // Carry height from the upwind cell for stability.
            let h_upwind = if u > 0.0 {
                self.heights[i - 1]
            } else {
                self.heights[i]
            };
            self.flux[i] = u * h_upwind;
        }

        // 3. Update column heights from the net flux out of each cell.
        for i in 0..NUM_CELLS {
            let net = (self.flux[i + 1] - self.flux[i]) / DX;
            self.heights[i] = (self.heights[i] - dt * net).max(MIN_HEIGHT);
        }
    }

    // --- Read-only accessors for the renderer (state, never internals) ---

    /// Read-only view of the water column heights, left to right.
    pub fn heights(&self) -> &[f32] {
        &self.heights
    }

    /// Number of water columns.
    pub fn num_cells(&self) -> usize {
        self.heights.len()
    }
}

impl Default for WaterSim {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience: full turn in radians, handy for callers building oscillations.
pub const TAU: f32 = 2.0 * PI;

#[cfg(test)]
mod tests {
    use super::*;

    fn total_water(sim: &WaterSim) -> f32 {
        sim.heights().iter().sum()
    }

    #[test]
    fn conserves_volume_under_tilt() {
        let mut sim = WaterSim::new();
        let start = total_water(&sim);
        // Hold a steady tilt for a few seconds of sim time.
        for _ in 0..2000 {
            sim.step(DT, 0.12);
        }
        let end = total_water(&sim);
        // The only sink is the MIN_HEIGHT clamp, which shouldn't trigger with
        // these constants — so volume should hold very tightly.
        assert!(
            (end - start).abs() / start < 1e-3,
            "volume drifted: {start} -> {end}"
        );
    }

    #[test]
    fn stays_finite_and_stable() {
        let mut sim = WaterSim::new();
        // Drive a full oscillation and make sure nothing blows up.
        for k in 0..5000 {
            let tilt = AUTO_MAX_ANGLE_TEST * (k as f32 * 0.02).sin();
            sim.step(DT, tilt);
        }
        for &h in sim.heights() {
            assert!(h.is_finite(), "height went non-finite");
            assert!(h > 0.0 && h < 10.0 * REST_HEIGHT, "height blew up: {h}");
        }
    }

    #[test]
    fn tilt_pushes_water_to_the_low_end() {
        let mut sim = WaterSim::new();
        // Positive tilt tips +x down; water should pile toward the right.
        for _ in 0..1500 {
            sim.step(DT, 0.12);
        }
        let h = sim.heights();
        let left: f32 = h[..h.len() / 4].iter().sum();
        let right: f32 = h[3 * h.len() / 4..].iter().sum();
        assert!(right > left, "expected more water on the right: L={left} R={right}");
    }

    const AUTO_MAX_ANGLE_TEST: f32 = 0.12;
}
