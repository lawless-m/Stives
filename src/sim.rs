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
/// `1.0` would be no damping. At the default `DT` (~250 substeps/sec) this
/// works out to roughly a quarter of the wave energy lost per second, so a
/// slosh visibly settles over a few seconds rather than ringing on.
pub const DAMPING: f32 = 0.999;

/// Turbulence forcing strength. Each substep adds a random velocity kick scaled
/// by the *local* flow speed, so moving water roughens into chop while still
/// water stays glassy and settles flat. The kick is smoothed across cells (see
/// `CHOP_SMOOTH`) so it drives rolling ripples, not grid-scale buzz. A pure 1D
/// shallow-water scheme is otherwise perfectly laminar. `0.0` restores it.
pub const TURBULENCE: f32 = 1.2;

/// Spatial smoothing for the turbulence kick — a 1-pole low-pass swept across
/// the cells each step. Lower = smoother, longer-wavelength chop; `1.0` = raw
/// per-cell white noise (the buzzy, high-frequency leading-edge look).
pub const CHOP_SMOOTH: f32 = 0.2;

/// Width, in cells, of the absorbing "sponge" zone at each wall.
const SPONGE_CELLS: usize = 12;

/// Peak extra per-substep velocity damping right at a wall, ramping to none at
/// the inner edge of the sponge. The walls reflect (volume is still conserved),
/// but reflected waves lose a little energy and scatter, so the surface no
/// longer rings with mirror-perfect standing waves. `0.0` = perfect mirror.
pub const WALL_ABSORPTION: f32 = 0.06;

/// Spike control: a conservative height diffusion that shaves steep one-cell
/// overshoots — wall run-up jets and the crest of a near-vertical slosh front.
/// It engages only where the jump between adjacent columns exceeds
/// `SHOCK_THRESHOLD`, so the broad slosh crest and the surface chop (both gentle
/// gradients) pass through untouched. Flux form with no-flux walls conserves
/// volume exactly. `0.0` disables it.
pub const SHOCK_SMOOTH: f32 = 0.3;

/// Adjacent-column height jump (sim units) at which `SHOCK_SMOOTH` begins to
/// engage, ramping to full at twice this. Set above the gradients of normal
/// waves and chop so only true spikes and steep fronts are touched.
pub const SHOCK_THRESHOLD: f32 = 0.1;

/// Hyperviscosity coefficient — a biharmonic (∇⁴) filter applied to the
/// *velocity* field each substep. This staggered scheme is dispersive: the
/// grid-scale (2-cell) waves travel at the wrong speed and get shed as ripples
/// off steep fronts. A ∇⁴ filter damps ∝ k⁴, so it scrubs that grid noise hard
/// while leaving the resolved slosh and chop essentially untouched (a plain ∇²
/// would smear them). Acting on velocity rather than height keeps water volume
/// exactly conserved. Must stay under the stability limit ~1/16; `0.0` disables.
pub const HYPERVISCOSITY: f32 = 0.04;

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
    /// Scratch buffer for the hyperviscosity filter (holds ∇² of velocity
    /// between the two biharmonic passes).
    scratch: Vec<f32>,
    /// State for the internal PRNG that drives the turbulence forcing.
    rng: u32,
}

impl WaterSim {
    /// A calm, flat, full tank.
    pub fn new() -> Self {
        Self {
            heights: vec![REST_HEIGHT; NUM_CELLS],
            vel: vec![0.0; NUM_CELLS + 1],
            flux: vec![0.0; NUM_CELLS + 1],
            scratch: vec![0.0; NUM_CELLS + 1],
            rng: 0x9E3779B9,
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
        // Smoothed noise, low-passed as we sweep left→right so the turbulence
        // drives longer-wavelength chop rather than grid-scale buzz.
        let mut chop = 0.0_f32;
        for i in 1..NUM_CELLS {
            let height_gradient = (self.heights[i] - self.heights[i - 1]) / DX;
            // Pressure (restoring) force pushes from tall columns to short ones;
            // the tilt term drives the bulk slosh.
            let mut u = self.vel[i] + dt * (-GRAVITY * height_gradient + g_horizontal);
            // Turbulence: motion breeds chop. A smoothed, flow-scaled random kick
            // roughens moving water into rolling ripples; still water stays glassy.
            chop += CHOP_SMOOTH * (self.next_noise() - chop);
            u += dt * TURBULENCE * u.abs() * chop;
            u *= DAMPING;
            // Sponge layer: extra damping ramping up toward the nearest wall, so
            // reflections come back softened and scattered, not mirror-perfect.
            let d_wall = i.min(NUM_CELLS - i);
            if d_wall < SPONGE_CELLS {
                let ramp = 1.0 - d_wall as f32 / SPONGE_CELLS as f32;
                u *= 1.0 - WALL_ABSORPTION * ramp;
            }
            self.vel[i] = u;
        }

        // 1b. Hyperviscosity: a biharmonic (∇⁴) filter on velocity that scrubs
        //     the grid-scale dispersive ripples off steep fronts while leaving
        //     resolved motion alone. Walls (faces 0 and NUM_CELLS) stay 0.
        self.scratch[0] = 0.0;
        self.scratch[NUM_CELLS] = 0.0;
        for i in 1..NUM_CELLS {
            self.scratch[i] = self.vel[i - 1] - 2.0 * self.vel[i] + self.vel[i + 1];
        }
        for i in 1..NUM_CELLS {
            let lap2 = self.scratch[i - 1] - 2.0 * self.scratch[i] + self.scratch[i + 1];
            self.vel[i] -= HYPERVISCOSITY * lap2;
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

        // 4. Spike control: shave steep one-cell overshoots — wall run-up jets
        //    and the crest of a near-vertical slosh front — with a conservative
        //    height diffusion that engages only on steep jumps. Flux form
        //    (no-flux walls, stored in scratch) keeps total volume exact.
        for i in 0..NUM_CELLS - 1 {
            let jump = self.heights[i] - self.heights[i + 1];
            // Ramp in over steep jumps only; gentle gradients (waves, chop) give
            // steep = 0 and pass untouched.
            let steep = ((jump.abs() - SHOCK_THRESHOLD) / SHOCK_THRESHOLD).clamp(0.0, 1.0);
            // Store in `scratch`, not `flux`: clobbering flux[0] would leave a
            // stale nonzero flux on the left wall for the next step's transport.
            self.scratch[i] = SHOCK_SMOOTH * steep * jump; // conservative flux cell i -> i+1
        }
        let mut left_flux = 0.0_f32;
        for i in 0..NUM_CELLS {
            let right_flux = if i < NUM_CELLS - 1 { self.scratch[i] } else { 0.0 };
            self.heights[i] += left_flux - right_flux;
            left_flux = right_flux;
        }
    }

    /// Internal xorshift32 PRNG → uniform noise in `[-1, 1]`, driving the
    /// turbulence forcing. Kept self-contained so this module stays free of any
    /// `rand`/rendering dependency (the architecture rule above).
    fn next_noise(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
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
