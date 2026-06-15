# Water Tank

A side-on water-sloshing **toy** — a tank of water on a slow see-saw tilt that
generates waves you watch. No game, no goal: set it running and watch. Inspired
by a powered see-saw water sculpture.

This is **Phase 1** as specified in `water-tank-docs.zip` (Rust + Macroquad, CPU
simulation, native build).

## Run

```sh
cargo run --release
```

(Native window. On Linux you'll need the usual OpenGL/X11 or Wayland dev
libraries that Macroquad links against.)

## Controls

- **Drag the slider** to tip the tank to an absolute angle — this hands control
  to *manual* mode. The water sloshes toward the low end and settles level.
- **Mode** button toggles between the self-running **Auto** see-saw and
  **Manual**.
- **Calm** button flattens the water.

By default it starts in Auto: a slow sine see-saw — just leave it running.

## Architecture

One non-negotiable rule (`docs/03-architecture.md`): **the simulation knows
nothing about how it is drawn.** The code keeps three independent layers, wired
together only in `main.rs` (`read input → step sim → render`):

| Module        | Responsibility                                               |
| ------------- | ------------------------------------------------------------ |
| `src/sim.rs`  | 1D shallow-water physics over a flat array. No rendering.    |
| `src/tilt.rs` | The tilt input layer (auto see-saw / manual slider).         |
| `src/render.rs` | Side-on renderer — reads sim state read-only and draws it. |
| `src/main.rs` | Main loop + minimal immediate-mode UI.                        |

This split is what keeps the future Phase 2 isometric upgrade additive (widen
the sim to 2D, add a *separate* iso renderer) rather than a rewrite. Phase 2 is
**not** built here.

## Tuning

All the knobs are named constants at the top of `src/sim.rs` (cell count,
gravity, damping, timestep) and `src/tilt.rs` (auto period, max angles). Stability
is favoured over physical accuracy; the defaults are well inside the CFL limit.

## Tests

```sh
cargo test
```

Headless checks on the solver: volume conservation under tilt, stability under a
full oscillation, and that the tilt drives water to the low end.
