// ===================================================================
//  src/modes/reaction_diffusion.rs
// -------------------------------------------------------------------
//  Gray-Scott reaction-diffusion simulation.
//
//  WHY THIS IS SIMILAR TO METABALLS (but completely different)
//  ─────────────────────────────────────────────────────────────
//  Metaballs compute a scalar field per pixel (sum of r²/d² from
//  each ball), then threshold and shade it. Reaction-diffusion also
//  maintains a per-pixel scalar field (two chemical concentrations
//  A and B), but the field *evolves through time* via diffusion and
//  a nonlinear reaction. The result is physics-driven pattern
//  formation rather than geometry-driven blobs.
//
//  THE CHEMISTRY
//  ─────────────────────────────────────────────────────────────
//  Two chemical species, A (activator) and B (inhibitor):
//
//      A + 2B  →  3B        (auto-catalytic reaction)
//      B       →  P         (spontaneous decay of B)
//
//  Update equations per cell per time step:
//
//    dA/dt = Da·∇²A  -  A·B²  +  f·(1 - A)
//    dB/dt = Db·∇²B  +  A·B²  -  (f + k)·B
//
//  Where:
//    ∇²  = discrete Laplacian (weighted 9-point stencil)
//    Da  = diffusion rate of A  (typically ~1.0)
//    Db  = diffusion rate of B  (typically ~0.5, B diffuses slower)
//    f   = feed rate  (A is pumped in at this rate)
//    k   = kill rate  (B dies at this rate)
//
//  Different (f, k) pairs produce spectacularly different patterns:
//    CORAL   f=0.054, k=0.062  → spots and coral-like growths
//    STRIPES f=0.060, k=0.062  → fingerprint whorls
//    MAZES   f=0.029, k=0.057  → wandering maze-like lines
//    BUBBLES f=0.010, k=0.047  → large circular bubbles
//    WORMS   f=0.078, k=0.061  → tangled worm-like forms
//
//  The mode cycles through these presets every ~40 seconds.
//
//  RENDERING
//  B concentration → brightness → colour (via ColorProvider tint).
//  Half-block (▀) rendering gives double the vertical resolution.
//
//  PERFORMANCE NOTE
//  The simulation grid is capped at 160×60 cells regardless of
//  terminal size, then upscaled to fill. This keeps the per-frame
//  computation constant (~576,000 cell updates/frame at 6 steps/frame)
//  so it runs smoothly even on large terminals.
//
//  impl ModeDescriptor for ReactionDiffusionMode {
//      const ID:   &'static str = "rdiffusion";
//      const NAME: &'static str = "Reaction Diffusion";
//      const DESC: &'static str = "Gray-Scott chemical pattern formation";
//      const FPS:  u32          = 50;
//  }
// ===================================================================

use crate::ansi::RESET;
use crate::color::{ColorProvider, ColorMode};
use crate::mode_base::Mode;
use rand::RngExt;

type Rgb = (u8, u8, u8);

// ── Simulation constants ──────────────────────────────────────────────────────

const SIM_W: usize = 160; // simulation grid width (upscaled to fill terminal)
const SIM_H: usize = 80;  // simulation grid height (× 2 for half-blocks)

const DA: f32 = 1.00; // diffusion rate of A
const DB: f32 = 0.50; // diffusion rate of B (always slower than A)
const DT: f32 = 1.00; // time step (1 = fast evolution)
const STEPS_PER_FRAME: usize = 8; // simulation steps to run each rendered frame

// Parameter presets — each gives a completely different pattern family.
#[derive(Clone, Copy)]
struct Preset {
    f: f32, // feed rate
    k: f32, // kill rate
    name: &'static str,
}

const PRESETS: &[Preset] = &[
    Preset { f: 0.0545, k: 0.062,  name: "Coral"   }, // spots → coral
    Preset { f: 0.0600, k: 0.0620, name: "Stripes"  }, // fingerprints
    Preset { f: 0.0290, k: 0.0570, name: "Maze"     }, // maze lines
    Preset { f: 0.0100, k: 0.0470, name: "Bubbles"  }, // large bubbles
    Preset { f: 0.0780, k: 0.0610, name: "Worms"    }, // tangled worms
];
const PRESET_DURATION: f64 = 42.0; // seconds per preset

// ── Mode ──────────────────────────────────────────────────────────────────────

pub struct ReactionDiffusionMode {
    speed: f64,
    color: ColorProvider,

    grid_a: Vec<f32>, // concentration of A per cell
    grid_b: Vec<f32>, // concentration of B per cell
    buf_a:  Vec<f32>, // double-buffer scratch
    buf_b:  Vec<f32>,

    preset_idx:   usize,
    preset_timer: f64,
    transition:   f32, // 0..1 blend during preset crossfade
    prev_a:       Vec<f32>, // snapshot for crossfade blend
    prev_b:       Vec<f32>,

    initialized: bool,
}

impl ReactionDiffusionMode {
    pub fn new(speed: f64, color: ColorProvider) -> Self {
        let cells = SIM_W * SIM_H;
        Self {
            speed, color,
            grid_a: vec![1.0; cells],
            grid_b: vec![0.0; cells],
            buf_a:  vec![0.0; cells],
            buf_b:  vec![0.0; cells],
            preset_idx: 0,
            preset_timer: 0.0,
            transition: 0.0,
            prev_a: vec![1.0; cells],
            prev_b: vec![0.0; cells],
            initialized: false,
        }
    }

    /// Seed the grid with a number of small perturbation zones that
    /// act as nucleation sites for pattern formation.
    fn seed(&mut self) {
        let cells = SIM_W * SIM_H;
        self.grid_a = vec![1.0; cells];
        self.grid_b = vec![0.0; cells];

        let mut rng = rand::rng();
        let num_seeds = rng.random_range(8..20);
        for _ in 0..num_seeds {
            let cx = rng.random_range(4..SIM_W.saturating_sub(4));
            let cy = rng.random_range(4..SIM_H.saturating_sub(4));
            let r  = rng.random_range(2..5usize);
            for dy in -(r as i32)..=(r as i32) {
                for dx in -(r as i32)..=(r as i32) {
                    let nx = (cx as i32 + dx) as usize;
                    let ny = (cy as i32 + dy) as usize;
                    if nx < SIM_W && ny < SIM_H {
                        let i = ny * SIM_W + nx;
                        self.grid_b[i] = 0.25 + rng.random_range(-0.05..0.05);
                        self.grid_a[i] = 0.50 + rng.random_range(-0.05..0.05);
                    }
                }
            }
        }
        self.initialized = true;
    }

    /// Run one simulation step.
    fn step(&mut self) {
        let p = PRESETS[self.preset_idx];

        for y in 0..SIM_H {
            for x in 0..SIM_W {
                let a = self.grid_a[y * SIM_W + x];
                let b = self.grid_b[y * SIM_W + x];

                // Weighted 9-point Laplacian (sums to 0 for uniform fields)
                let lap_a = laplacian(&self.grid_a, x, y);
                let lap_b = laplacian(&self.grid_b, x, y);

                let reaction = a * b * b;

                self.buf_a[y * SIM_W + x] =
                    (a + (DA * lap_a - reaction + p.f * (1.0 - a)) * DT).clamp(0.0, 1.0);
                self.buf_b[y * SIM_W + x] =
                    (b + (DB * lap_b + reaction - (p.f + p.k) * b) * DT).clamp(0.0, 1.0);
            }
        }
        std::mem::swap(&mut self.grid_a, &mut self.buf_a);
        std::mem::swap(&mut self.grid_b, &mut self.buf_b);
    }

    /// Map a B concentration (0..1) to an RGB colour for the given ColorMode.
    fn b_to_rgb(&self, b: f32, t_abs: f64, x: usize, y: usize) -> Rgb {
        let v = b.clamp(0.0, 1.0) as f64;
        match self.color.mode {
            ColorMode::Matrix => {
                let g = (v * 255.0) as u8;
                (0, g, (g as f64 * 0.3) as u8)
            }
            ColorMode::Ocean => {
                // Dark blue → bright teal
                lerp((4, 12, 38), (40, 220, 200), v)
            }
            ColorMode::Sunset => {
                // Deep crimson → bright orange
                lerp((24, 4, 4), (255, 140, 40), v)
            }
            _ => {
                // Rainbow: hue rotates with B value + slow time drift + position
                let hue = (v * 0.75 + t_abs * 0.04
                    + x as f64 * 0.001 + y as f64 * 0.0015)
                    .rem_euclid(1.0);
                hue_to_rgb(hue, v)
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Weighted 9-point discrete Laplacian with toroidal (wrap-around) boundaries.
#[inline]
fn laplacian(grid: &[f32], x: usize, y: usize) -> f32 {
    let w = SIM_W as i32;
    let h = SIM_H as i32;
    let at = |nx: i32, ny: i32| -> f32 {
        let nx = nx.rem_euclid(w) as usize;
        let ny = ny.rem_euclid(h) as usize;
        grid[ny * SIM_W + nx]
    };
    let (ix, iy) = (x as i32, y as i32);
    let center = grid[y * SIM_W + x];
    -center
    + 0.20 * (at(ix+1,iy) + at(ix-1,iy) + at(ix,iy+1) + at(ix,iy-1))
    + 0.05 * (at(ix+1,iy+1) + at(ix+1,iy-1) + at(ix-1,iy+1) + at(ix-1,iy-1))
}

fn lerp(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let c = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * t) as u8;
    (c(a.0,b.0), c(a.1,b.1), c(a.2,b.2))
}

/// HSV-style rainbow mapping: hue 0..1 → RGB, modulated by brightness v.
fn hue_to_rgb(hue: f64, v: f64) -> Rgb {
    let h6 = (hue * 6.0).rem_euclid(6.0);
    let (r, g, b) = if      h6 < 1.0 { (1.0, h6,        0.0) }
                    else if h6 < 2.0 { (2.0-h6, 1.0,     0.0) }
                    else if h6 < 3.0 { (0.0, 1.0,        h6-2.0) }
                    else if h6 < 4.0 { (0.0, 4.0-h6,     1.0) }
                    else if h6 < 5.0 { (h6-4.0, 0.0,     1.0) }
                    else             { (1.0, 0.0,         6.0-h6) };
    // Boost saturation near B=0.5 (the reaction boundary is most vivid there)
    let sat  = (v * 2.0).min(1.0);
    let lum  = v * 0.9 + 0.1;
    ((r * sat * lum * 255.0) as u8,
     (g * sat * lum * 255.0) as u8,
     (b * sat * lum * 255.0) as u8)
}

// ── Mode impl ─────────────────────────────────────────────────────────────────

impl Mode for ReactionDiffusionMode {
    fn update(&mut self, dt: f64, _width: u16, _height: u16, _t: f64) {
        if !self.initialized { self.seed(); }

        let dt = dt * self.speed;
        self.preset_timer += dt;

        // ── Preset cycling ────────────────────────────────────────────────────
        if self.preset_timer >= PRESET_DURATION {
            self.preset_timer = 0.0;
            // Snapshot current state for crossfade
            self.prev_a = self.grid_a.clone();
            self.prev_b = self.grid_b.clone();
            // Advance to next preset
            self.preset_idx = (self.preset_idx + 1) % PRESETS.len();
            // Re-seed a few zones to help the new pattern bootstrap
            let mut rng = rand::rng();
            for _ in 0..4 {
                let cx = rng.random_range(4..SIM_W.saturating_sub(4));
                let cy = rng.random_range(4..SIM_H.saturating_sub(4));
                for dy in -2i32..=2 {
                    for dx in -2i32..=2 {
                        let nx = (cx as i32 + dx) as usize;
                        let ny = (cy as i32 + dy) as usize;
                        if nx < SIM_W && ny < SIM_H {
                            let i = ny * SIM_W + nx;
                            self.grid_b[i] = 0.25;
                            self.grid_a[i] = 0.50;
                        }
                    }
                }
            }
            self.transition = 1.0;
        }

        if self.transition > 0.0 {
            self.transition = (self.transition - dt as f32 * 0.8).max(0.0);
        }

        // ── Simulation steps ──────────────────────────────────────────────────
        let steps = (STEPS_PER_FRAME as f64 * self.speed.clamp(0.5, 2.0)) as usize;
        for _ in 0..steps.min(15) { self.step(); }
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let tw = width  as usize;
        let th = height as usize;
        let ph = th * 2; // pixel height (half-block)

        // Scale factors to map terminal pixels → simulation cells
        let scale_x = SIM_W as f64 / tw as f64;
        let scale_y = SIM_H as f64 / ph as f64;

        let mut out = String::with_capacity(tw * th * 44);

        for row in 0..th {
            for col in 0..tw {
                // Sample two pixel rows per terminal row for upper/lower half-block
                let sy0 = ((row * 2    ) as f64 * scale_y) as usize;
                let sy1 = ((row * 2 + 1) as f64 * scale_y) as usize;
                let sx  = (col as f64 * scale_x) as usize;

                let i0 = sy0.min(SIM_H-1) * SIM_W + sx.min(SIM_W-1);
                let i1 = sy1.min(SIM_H-1) * SIM_W + sx.min(SIM_W-1);

                let mut b0 = self.grid_b[i0];
                let mut b1 = self.grid_b[i1];

                // Blend with previous state during crossfade
                if self.transition > 0.0 {
                    let t = self.transition;
                    b0 = b0 * (1.0 - t) + self.prev_b[i0] * t;
                    b1 = b1 * (1.0 - t) + self.prev_b[i1] * t;
                }

                let (ur, ug, ub) = self.b_to_rgb(b0, t_abs, sx, sy0);
                let (lr, lg, lb) = self.b_to_rgb(b1, t_abs, sx, sy1);

                out.push_str(&format!(
                    "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m\u{2580}",
                    ur, ug, ub, lr, lg, lb,
                ));
            }
            out.push_str(RESET);
            if row < th - 1 { out.push('\n'); }
        }

        // Preset name overlay
        let p = PRESETS[self.preset_idx];
        let progress = (self.preset_timer / PRESET_DURATION).clamp(0.0, 1.0);
        let bar_w    = 20usize;
        let filled   = (progress * bar_w as f64) as usize;
        let bar: String = (0..bar_w).map(|i| if i < filled { '█' } else { '░' }).collect();
        out.push_str(&format!(
            "\x1b[1;2H\x1b[48;2;8;8;14m\x1b[38;2;180;200;255m {:<8} {}{}\x1b[0m",
            p.name, bar, RESET,
        ));

        out
    }
}
