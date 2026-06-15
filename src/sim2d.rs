//! 2D shallow-water simulation (Phase 2, docs/04-phase2-isometric.md).
//!
//! The Phase 1 widening: the same shallow-water equations as `sim.rs`, but over
//! a 2D grid with a second horizontal velocity component, so waves propagate in
//! two directions and reflect off four walls. As in 1D, this module knows
//! *nothing* about how it is drawn (docs/03-architecture.md) — it owns the grid
//! and steps physics given a 2D tilt.
//!
//! Reuses the shared physical constants from `sim` (gravity, damping, timestep,
//! rest height) so the two solvers stay in step; only the grid shape is local.

use crate::sim::{DAMPING, GRAVITY, REST_HEIGHT};

// ----------------------------------------------------------------------------
// Tunables (docs/04-phase2-isometric.md: CPU is fine up to ~256×256 @ 60fps).
// ----------------------------------------------------------------------------

/// Grid width and height in cells. Square tank.
pub const NX: usize = 96;
pub const NY: usize = 96;

/// Physical tank extent in sim units (square). Cell spacing derives from this.
pub const TANK_SIZE: f32 = 4.0;

/// Cell spacing in each axis.
pub const DX: f32 = TANK_SIZE / NX as f32;
pub const DY: f32 = TANK_SIZE / NY as f32;

/// Minimum column height — small positivity floor for stability.
const MIN_HEIGHT: f32 = 0.02;

/// 2D shallow-water state on a staggered grid.
///
/// - `h[j*NX + i]` — column height of cell `(i, j)`            (NX·NY)
/// - `u` — x-velocity on vertical faces, `(NX+1)` per row      ((NX+1)·NY)
/// - `v` — y-velocity on horizontal faces, `NX` per row        (NX·(NY+1))
///
/// Face velocities on the four walls are never written and stay `0.0`, giving
/// reflecting boundaries and exact volume conservation (interior fluxes
/// telescope; nothing flows through the walls).
pub struct WaterSim2d {
    h: Vec<f32>,
    u: Vec<f32>,
    v: Vec<f32>,
    flux_x: Vec<f32>,
    flux_y: Vec<f32>,
}

#[inline]
fn cell(i: usize, j: usize) -> usize {
    j * NX + i
}
#[inline]
fn uface(i: usize, j: usize) -> usize {
    j * (NX + 1) + i
}
#[inline]
fn vface(i: usize, j: usize) -> usize {
    j * NX + i
}

impl WaterSim2d {
    /// A calm, flat, full tank.
    pub fn new() -> Self {
        Self {
            h: vec![REST_HEIGHT; NX * NY],
            u: vec![0.0; (NX + 1) * NY],
            v: vec![0.0; NX * (NY + 1)],
            flux_x: vec![0.0; (NX + 1) * NY],
            flux_y: vec![0.0; NX * (NY + 1)],
        }
    }

    /// Flatten the water and kill all motion (the "calm" reset).
    pub fn reset(&mut self) {
        self.h.iter_mut().for_each(|h| *h = REST_HEIGHT);
        self.u.iter_mut().for_each(|u| *u = 0.0);
        self.v.iter_mut().for_each(|v| *v = 0.0);
    }

    /// Advance the physics by `dt` seconds under a 2D tilt (radians on each
    /// axis). Positive `tilt_x`/`tilt_y` push water toward `+x` / `+y`.
    pub fn step(&mut self, dt: f32, tilt_x: f32, tilt_y: f32) {
        let gx = GRAVITY * tilt_x.sin();
        let gy = GRAVITY * tilt_y.sin();

        // 1a. x-velocities on interior vertical faces.
        for j in 0..NY {
            for i in 1..NX {
                let dh = (self.h[cell(i, j)] - self.h[cell(i - 1, j)]) / DX;
                let f = uface(i, j);
                self.u[f] = (self.u[f] + dt * (-GRAVITY * dh + gx)) * DAMPING;
            }
        }
        // 1b. y-velocities on interior horizontal faces.
        for j in 1..NY {
            for i in 0..NX {
                let dh = (self.h[cell(i, j)] - self.h[cell(i, j - 1)]) / DY;
                let f = vface(i, j);
                self.v[f] = (self.v[f] + dt * (-GRAVITY * dh + gy)) * DAMPING;
            }
        }

        // 2a. Upwind x-flux on interior faces.
        for j in 0..NY {
            for i in 1..NX {
                let f = uface(i, j);
                let uu = self.u[f];
                let h_up = if uu > 0.0 {
                    self.h[cell(i - 1, j)]
                } else {
                    self.h[cell(i, j)]
                };
                self.flux_x[f] = uu * h_up;
            }
        }
        // 2b. Upwind y-flux on interior faces.
        for j in 1..NY {
            for i in 0..NX {
                let f = vface(i, j);
                let vv = self.v[f];
                let h_up = if vv > 0.0 {
                    self.h[cell(i, j - 1)]
                } else {
                    self.h[cell(i, j)]
                };
                self.flux_y[f] = vv * h_up;
            }
        }

        // 3. Update heights from net flux out of each cell.
        for j in 0..NY {
            for i in 0..NX {
                let net = (self.flux_x[uface(i + 1, j)] - self.flux_x[uface(i, j)]) / DX
                    + (self.flux_y[vface(i, j + 1)] - self.flux_y[vface(i, j)]) / DY;
                let c = cell(i, j);
                self.h[c] = (self.h[c] - dt * net).max(MIN_HEIGHT);
            }
        }
    }

    // --- Read-only accessors for the renderer ---

    /// Row-major view of the column heights (`j*NX + i`).
    pub fn heights(&self) -> &[f32] {
        &self.h
    }
    pub fn nx(&self) -> usize {
        NX
    }
    pub fn ny(&self) -> usize {
        NY
    }
}

impl Default for WaterSim2d {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn total(sim: &WaterSim2d) -> f32 {
        sim.heights().iter().sum()
    }

    #[test]
    fn conserves_volume_under_tilt() {
        let mut sim = WaterSim2d::new();
        let start = total(&sim);
        for _ in 0..1500 {
            sim.step(crate::sim::DT, 0.08, 0.05);
        }
        let end = total(&sim);
        assert!((end - start).abs() / start < 1e-3, "drift {start} -> {end}");
    }

    #[test]
    fn stays_finite_and_stable() {
        let mut sim = WaterSim2d::new();
        for k in 0..3000 {
            let a = k as f32 * 0.01;
            sim.step(crate::sim::DT, 0.1 * a.cos(), 0.1 * a.sin());
        }
        for &h in sim.heights() {
            assert!(h.is_finite() && h > 0.0 && h < 10.0 * REST_HEIGHT, "blew up: {h}");
        }
    }

    #[test]
    fn tilt_pushes_water_to_the_corner() {
        let mut sim = WaterSim2d::new();
        // Tilt toward +x and +y: water should gather in the far +x/+y corner.
        for _ in 0..1500 {
            sim.step(crate::sim::DT, 0.1, 0.1);
        }
        let h = sim.heights();
        let near = h[cell(0, 0)];
        let far = h[cell(NX - 1, NY - 1)];
        assert!(far > near, "expected water in the +x/+y corner: near={near} far={far}");
    }
}
