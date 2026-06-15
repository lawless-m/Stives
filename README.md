# Water Tank

A side-on water-sloshing **toy** — a tank of water on a slow see-saw tilt that
generates waves you watch. No game, no goal: set it running and watch. Inspired
by a powered see-saw water sculpture.

Built with Rust + Macroquad, CPU simulation, native build (per
`water-tank-docs.zip`). Both phases are present and selected at runtime:

- **Phase 1 — side-on** (default): a 1D shallow-water tank seen from the side.
- **Phase 2 — isometric** (`2d` flag): a 2D shallow-water grid drawn as a cheap
  isometric field of coloured diamonds; the auto tilt *precesses* so waves chase
  around the tank.

## Run

```sh
cargo run --release          # Phase 1, side-on
cargo run --release -- 2d    # Phase 2, isometric (also accepts: iso, --2d, --iso)
```

(Native window. On Linux you'll need the usual OpenGL/X11 or Wayland dev
libraries that Macroquad links against.)

## Controls

**Side-on (default):**
- **Drag the slider** to tip the tank to an absolute angle — hands control to
  *manual* mode. Water sloshes to the low end and settles level.
- **Mode** toggles **Auto** see-saw / **Manual**. **Calm** flattens the water.

**Isometric (`2d`):**
- **Drag in the tilt pad** (bottom-left) to tip the tank in two directions at
  once. Deep water is dark blue, crests are light.
- **Mode** toggles **Auto** (precession) / **Manual**. **Calm** flattens it.

Both start in Auto — just leave it running.

## Architecture

One non-negotiable rule (`docs/03-architecture.md`): **the simulation knows
nothing about how it is drawn.** The code keeps three independent layers, wired
together only in `main.rs` (`read input → step sim → render`):

| Module             | Responsibility                                          |
| ------------------ | ------------------------------------------------------- |
| `src/sim.rs`       | 1D shallow-water physics over a flat array.             |
| `src/sim2d.rs`     | 2D shallow-water grid (Phase 2). Reuses sim.rs physics constants. |
| `src/tilt.rs`      | Tilt input layer — 1D slider/see-saw and 2D pad/precession. |
| `src/render.rs`    | Side-on renderer — reads sim state read-only, draws it. |
| `src/render_iso.rs`| Isometric renderer (Phase 2) — separate, reads 2D state read-only. |
| `src/main.rs`      | Mode selection + run loops + minimal immediate-mode UI. |

Phase 2 was added exactly as the rule intended: the 2D sim is a widening of the
1D equations (a second velocity component), the iso renderer is a *separate*
module added alongside the side-on one, and **neither sim nor the side-on
renderer was touched** to make it work. The two renderers never see each other.

## Tuning

Named constants at the top of each module: `src/sim.rs` (1D cells, gravity,
damping, timestep), `src/sim2d.rs` (grid size NX·NY), `src/tilt.rs` (auto
period, precession period, max angles), and the renderers (colours, tile look).
Stability is favoured over physical accuracy; the defaults sit well inside the
CFL limit.

## Tests

```sh
cargo test
```

Headless checks on the solver: volume conservation under tilt, stability under a
full oscillation, and that the tilt drives water to the low end.
