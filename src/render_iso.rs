//! Isometric renderer (Phase 2, docs/04-phase2-isometric.md).
//!
//! The iso renderer: the 2D height grid drawn as a connected isometric
//! height-field mesh — quads between adjacent cell centres, raised by their
//! height so the surface undulates as one continuous sheet (no gaps). Colour
//! shows depth (deep = dark blue, crests = light) and a cheap fake light shades
//! the slopes for relief. No 3D camera or real normals. Pure 2D Macroquad.
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
const RELIEF_FRAC: f32 = 2.6;

/// Strength of the fake slope shading (brightens up-slopes, darkens backs).
const SHADE: f32 = 3.0;

/// Render-only smoothing passes over the displayed surface. Each 5-point blur
/// melts the grid-scale dispersive checkerboard the 2D solver sheds, leaving the
/// real waves. Cosmetic: the sim is never touched. `0` shows the raw grid.
const SMOOTH_PASSES: usize = 2;

/// Height range (relative to rest) mapped across the colour ramp. Tighter than
/// the physical swing so the waves show strong contrast.
const H_LO: f32 = 0.72 * REST_HEIGHT;
const H_HI: f32 = 1.28 * REST_HEIGHT;

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

    // Cosmetic 5-point blur of the displayed surface (sim untouched): melts the
    // grid-scale dispersive checkerboard into organic waves.
    let mut hs = h.to_vec();
    for _ in 0..SMOOTH_PASSES {
        let src = hs.clone();
        for j in 0..ny {
            for i in 0..nx {
                let c = j * nx + i;
                let l = if i > 0 { src[c - 1] } else { src[c] };
                let r = if i < nx - 1 { src[c + 1] } else { src[c] };
                let u = if j > 0 { src[c - nx] } else { src[c] };
                let d = if j < ny - 1 { src[c + nx] } else { src[c] };
                hs[c] = 0.5 * src[c] + 0.125 * (l + r + u + d);
            }
        }
    }
    let h = &hs[..];

    // Project a cell centre (raised by its height relief) to screen space.
    let project = |i: usize, j: usize| -> Vec2 {
        let height = h[j * nx + i];
        let x = origin_x + (i as f32 - j as f32) * half_tw;
        let off = (height / REST_HEIGHT - 1.0) * relief;
        let y = origin_y + (i + j) as f32 * half_th - off;
        vec2(x, y)
    };

    // Connected mesh: a quad between each block of four adjacent cell centres,
    // so the surface is gap-free and the relief reads as one undulating sheet.
    // Painter's order: back (small i+j) to front (large i+j).
    for s in 0..=(nx + ny - 4) {
        let i_start = s.saturating_sub(ny - 2);
        let i_end = s.min(nx - 2);
        for i in i_start..=i_end {
            let j = s - i;
            let a = project(i, j);
            let b = project(i + 1, j);
            let c = project(i + 1, j + 1);
            let d = project(i, j + 1);

            let h00 = h[j * nx + i];
            let h11 = h[(j + 1) * nx + i + 1];
            let avg = 0.25 * (h00 + h[j * nx + i + 1] + h11 + h[(j + 1) * nx + i]);
            let t = ((avg - H_LO) * inv_range).clamp(0.0, 1.0);
            let base = lerp_color(DEEP_COLOR, CREST_COLOR, t);

            // Fake light: brighten tiles whose surface rises toward the viewer
            // (front corner higher than back), darken the backs.
            let shade = (1.0 + SHADE * (h11 - h00)).clamp(0.5, 1.6);
            let color = Color::new(
                (base.r * shade).min(1.0),
                (base.g * shade).min(1.0),
                (base.b * shade).min(1.0),
                1.0,
            );

            draw_triangle(a, b, c, color);
            draw_triangle(a, c, d, color);
        }
    }
}
