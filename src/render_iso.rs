//! Isometric renderer (Phase 2, docs/04-phase2-isometric.md).
//!
//! The deliberately *cheap* iso renderer: the 2D height grid drawn as coloured
//! isometric diamonds, with height shown via colour (deep = dark blue, crests =
//! light) plus a small vertical pixel offset for relief. No 3D camera, no
//! normals, no lighting — the motion sells it as water. Pure 2D Macroquad.
//!
//! Like the side-on renderer, it only ever *reads* sim state, and it lives
//! entirely alongside `render.rs` — neither renderer touches the other, and
//! the sim is untouched by either (docs/03-architecture.md).

use macroquad::prelude::*;

use crate::sim::REST_HEIGHT;
use crate::sim2d::WaterSim2d;

// ----------------------------------------------------------------------------
// Drawing tunables (presentation only — no physics here).
// ----------------------------------------------------------------------------

/// Fraction of the screen the iso grid spans (fit to the tighter axis).
const FIT_FRAC: f32 = 0.86;
/// Isometric tile height as a fraction of its width (2:1 is the classic look).
const TILE_ASPECT: f32 = 0.5;
/// Vertical pixels of relief per unit of (height / rest), scaled by tile width.
const RELIEF_FRAC: f32 = 1.4;

/// Height range (relative to rest) mapped across the colour ramp.
const H_LO: f32 = 0.55 * REST_HEIGHT;
const H_HI: f32 = 1.55 * REST_HEIGHT;

const DEEP_COLOR: Color = Color::new(0.04, 0.16, 0.42, 1.0);
const CREST_COLOR: Color = Color::new(0.62, 0.88, 1.00, 1.0);
const BACKGROUND_COLOR: Color = Color::new(0.05, 0.06, 0.09, 1.0);

pub fn background_color() -> Color {
    BACKGROUND_COLOR
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        1.0,
    )
}

/// Draw the 2D water grid as an isometric field of coloured diamonds.
pub fn draw_tank(sim: &WaterSim2d) {
    let nx = sim.nx();
    let ny = sim.ny();
    let h = sim.heights();

    // Fit the iso footprint to the screen. The diamond field spans
    // (nx+ny) half-tiles wide and (nx+ny) half-tiles tall (before relief).
    let span = (nx + ny) as f32;
    let tw_from_w = screen_width() * FIT_FRAC * 2.0 / span;
    let tw_from_h = screen_height() * FIT_FRAC * 2.0 / (span * TILE_ASPECT);
    let tw = tw_from_w.min(tw_from_h);
    let th = tw * TILE_ASPECT;
    let relief = tw * RELIEF_FRAC;

    // Centre the field.
    let origin_x = screen_width() * 0.5;
    let origin_y = screen_height() * 0.5 - (nx + ny - 2) as f32 * th * 0.25;

    let half_tw = tw * 0.5;
    let half_th = th * 0.5;
    let inv_range = 1.0 / (H_HI - H_LO);

    // Painter's order: draw back-to-front by increasing (i + j) so nearer
    // (lower, raised) tiles overdraw farther ones.
    for s in 0..=(nx + ny - 2) {
        let i_start = s.saturating_sub(ny - 1);
        let i_end = s.min(nx - 1);
        for i in i_start..=i_end {
            let j = s - i;
            let height = h[j * nx + i];

            let cx = origin_x + (i as f32 - j as f32) * half_tw;
            let off = (height / REST_HEIGHT - 1.0) * relief;
            let cy = origin_y + (i + j) as f32 * half_th - off;

            let t = ((height - H_LO) * inv_range).clamp(0.0, 1.0);
            let color = lerp_color(DEEP_COLOR, CREST_COLOR, t);

            let top = vec2(cx, cy - half_th);
            let right = vec2(cx + half_tw, cy);
            let bottom = vec2(cx, cy + half_th);
            let left = vec2(cx - half_tw, cy);
            draw_triangle(top, right, bottom, color);
            draw_triangle(top, bottom, left, color);
        }
    }
}
