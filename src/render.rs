//! Side-on renderer (docs/02-phase1-spec.md, docs/03-architecture.md).
//!
//! Takes a *read-only* view of the sim state and draws it. It knows nothing
//! about the physics — it can draw any valid sim state without assuming how it
//! was produced. The whole tank is rotated by the tilt so it reads as a powered
//! see-saw; at rest the sloshed surface comes out level in the world, which is
//! exactly right. `tilt` is passed purely for this visual transform.
//!
//! Phase 2 will add a *separate* isometric renderer alongside this one; this
//! file stays untouched by that.

use macroquad::prelude::*;

use crate::sim::{self, WaterSim};

// ----------------------------------------------------------------------------
// Drawing tunables (presentation only — no physics here).
// ----------------------------------------------------------------------------

/// Fraction of the screen's smaller dimension left as a margin around the tank.
const SCREEN_MARGIN_FRAC: f32 = 0.12;
/// Tank drawing aspect ratio (width : height).
const TANK_ASPECT: f32 = 2.2;
/// Fraction of the tank's height that a *resting* (full) tank fills, leaving
/// headroom above for crests so big waves don't clip the rim.
const REST_FILL_FRAC: f32 = 0.55;

const WATER_COLOR: Color = Color::new(0.18, 0.52, 0.90, 1.0);
const WATER_SURFACE_COLOR: Color = Color::new(0.55, 0.80, 1.00, 1.0);
const TANK_COLOR: Color = Color::new(0.85, 0.88, 0.92, 1.0);
const TANK_FLOOR_COLOR: Color = Color::new(0.30, 0.33, 0.38, 1.0);
const BACKGROUND_COLOR: Color = Color::new(0.07, 0.08, 0.11, 1.0);

/// Background to clear to each frame.
pub fn background_color() -> Color {
    BACKGROUND_COLOR
}

/// Geometry of the (un-rotated) tank in screen space, plus the tilt to rotate
/// the whole thing by. Local coords have their origin at the tank centre, `x`
/// to the right and `y` up.
struct TankView {
    cx: f32,
    cy: f32,
    half_w: f32,
    half_h: f32,
    sin_t: f32,
    cos_t: f32,
}

impl TankView {
    fn new(tilt: f32) -> Self {
        let margin = SCREEN_MARGIN_FRAC * screen_width().min(screen_height());
        let avail_w = screen_width() - 2.0 * margin;
        let avail_h = screen_height() - 2.0 * margin;

        // Fit the tank's aspect ratio inside the available area.
        let mut w = avail_w;
        let mut h = w / TANK_ASPECT;
        if h > avail_h {
            h = avail_h;
            w = h * TANK_ASPECT;
        }

        TankView {
            cx: screen_width() * 0.5,
            cy: screen_height() * 0.5,
            half_w: w * 0.5,
            half_h: h * 0.5,
            // Rotate the rigid tank by -tilt about its centre so a positive
            // tilt tips the right end down (matching the sim's convention).
            sin_t: tilt.sin(),
            cos_t: tilt.cos(),
        }
    }

    /// Local tank coords (x right, y up, origin at centre) -> screen pixels.
    fn to_screen(&self, lx: f32, ly: f32) -> Vec2 {
        let rx = lx * self.cos_t + ly * self.sin_t;
        let ry = -lx * self.sin_t + ly * self.cos_t;
        vec2(self.cx + rx, self.cy - ry)
    }

    /// Local x for the centre of cell `i`.
    fn cell_x(&self, i: usize) -> f32 {
        let cell_w = (self.half_w * 2.0) / sim::NUM_CELLS as f32;
        -self.half_w + (i as f32 + 0.5) * cell_w
    }

    /// Local y of the water surface for a column height, clamped to the rim.
    fn surface_y(&self, height: f32) -> f32 {
        let fill = (height / sim::REST_HEIGHT) * REST_FILL_FRAC * (self.half_h * 2.0);
        (-self.half_h + fill).min(self.half_h)
    }

    fn floor_y(&self) -> f32 {
        -self.half_h
    }
}

/// Draw a filled quad (two triangles) given its four corners in *local* coords.
fn fill_quad(view: &TankView, a: Vec2, b: Vec2, c: Vec2, d: Vec2, color: Color) {
    let a = view.to_screen(a.x, a.y);
    let b = view.to_screen(b.x, b.y);
    let c = view.to_screen(c.x, c.y);
    let d = view.to_screen(d.x, d.y);
    draw_triangle(a, b, c, color);
    draw_triangle(a, c, d, color);
}

/// Draw a line between two points given in *local* coords.
fn local_line(view: &TankView, a: Vec2, b: Vec2, thickness: f32, color: Color) {
    let a = view.to_screen(a.x, a.y);
    let b = view.to_screen(b.x, b.y);
    draw_line(a.x, a.y, b.x, b.y, thickness, color);
}

/// Draw the tank and its water from a read-only view of the sim.
pub fn draw_tank(sim: &WaterSim, tilt: f32) {
    let view = TankView::new(tilt);
    let heights = sim.heights();
    let n = sim.num_cells();
    let floor = view.floor_y();

    // Fill the water as a strip of quads between consecutive column centres,
    // with end caps out to the walls so the fill meets the tank sides.
    let left_x = -view.half_w;
    let right_x = view.half_w;

    // Left cap.
    let y0 = view.surface_y(heights[0]);
    fill_quad(
        &view,
        vec2(left_x, floor),
        vec2(view.cell_x(0), floor),
        vec2(view.cell_x(0), y0),
        vec2(left_x, y0),
        WATER_COLOR,
    );

    // Interior strip.
    for (i, pair) in heights.windows(2).enumerate() {
        let xa = view.cell_x(i);
        let xb = view.cell_x(i + 1);
        let ya = view.surface_y(pair[0]);
        let yb = view.surface_y(pair[1]);
        fill_quad(
            &view,
            vec2(xa, floor),
            vec2(xb, floor),
            vec2(xb, yb),
            vec2(xa, ya),
            WATER_COLOR,
        );
    }

    // Right cap.
    let yn = view.surface_y(heights[n - 1]);
    fill_quad(
        &view,
        vec2(view.cell_x(n - 1), floor),
        vec2(right_x, floor),
        vec2(right_x, yn),
        vec2(view.cell_x(n - 1), yn),
        WATER_COLOR,
    );

    // Crisp surface line on top of the fill.
    let mut prev = vec2(left_x, view.surface_y(heights[0]));
    for (i, &h) in heights.iter().enumerate() {
        let cur = vec2(view.cell_x(i), view.surface_y(h));
        local_line(&view, prev, cur, 2.0, WATER_SURFACE_COLOR);
        prev = cur;
    }
    local_line(&view, prev, vec2(right_x, yn), 2.0, WATER_SURFACE_COLOR);

    // Floor (drawn as a thin band so the container reads as solid).
    local_line(
        &view,
        vec2(left_x, floor),
        vec2(right_x, floor),
        4.0,
        TANK_FLOOR_COLOR,
    );

    // Tank walls/rim outline.
    let tl = vec2(left_x, view.half_h);
    let tr = vec2(right_x, view.half_h);
    let bl = vec2(left_x, floor);
    let br = vec2(right_x, floor);
    local_line(&view, bl, tl, 3.0, TANK_COLOR);
    local_line(&view, br, tr, 3.0, TANK_COLOR);
    local_line(&view, bl, br, 3.0, TANK_COLOR);
}
