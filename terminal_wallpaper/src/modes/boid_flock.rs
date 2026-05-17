// ===================================================================
//  src/modes/boid_flock.rs
// -------------------------------------------------------------------
//  Craig Reynolds' Boids algorithm — emergent flocking from three
//  local rules applied to every agent each frame:
//
//    SEPARATION  steer away from neighbours that are too close
//    ALIGNMENT   match heading with the average heading of neighbours
//    COHESION    steer toward the average position of neighbours
//
//  With ~120 boids these rules produce a convincing murmuration —
//  the flock stretches, splits, spirals, and reforms continuously.
//
//  VISUAL DESIGN
//  Each boid writes into a half-block pixel buffer. A trail grid
//  decays each frame so boids leave short, fading streaks. Colour
//  is derived from velocity heading angle → hue rotation, then
//  tinted by the active ColorProvider. The result is a smoothly
//  moving mass of colour that shifts as the flock changes direction.
//
//  Occasionally a "scatter event" fires (simulated predator) which
//  temporarily adds a strong repulsion point, splitting the flock and
//  making it reform — providing visual variety in long sessions.
//
//  impl ModeDescriptor for BoidFlockMode {
//      const ID:   &'static str = "flock";
//      const NAME: &'static str = "Boid Flock";
//      const DESC: &'static str = "Emergent flocking behaviour";
//      const FPS:  u32          = 60;
//  }
// ===================================================================

use crate::ansi::RESET;
use crate::color::{ColorProvider, ColorMode};
use crate::mode_base::Mode;
use rand::RngExt;

type Rgb = (u8, u8, u8);

// ── Tuning ────────────────────────────────────────────────────────────────────

const NUM_BOIDS:      usize = 130;
const MAX_SPEED:      f64   = 18.0;  // pixels/sec
const MIN_SPEED:      f64   = 6.0;
const PERCEPTION:     f64   = 9.0;   // neighbour detection radius (pixels)
const SEP_RADIUS:     f64   = 3.5;   // push away when closer than this
const SEP_WEIGHT:     f64   = 1.8;
const ALIGN_WEIGHT:   f64   = 1.0;
const COHESION_WEIGHT:f64   = 0.9;
const NOISE_WEIGHT:   f64   = 0.25;
const TRAIL_DECAY:    f32   = 2.8;   // trail brightness units per second

// ── Boid ──────────────────────────────────────────────────────────────────────

struct Boid { x: f64, y: f64, vx: f64, vy: f64 }

// ── Mode ──────────────────────────────────────────────────────────────────────

pub struct BoidFlockMode {
    speed:  f64,
    color:  ColorProvider,
    boids:  Vec<Boid>,

    /// Trail grid (pixel-resolution, same dims as the half-block canvas).
    /// Each cell stores a brightness 0..1 and the RGB of the last boid
    /// that visited.
    trail_bright: Vec<f32>,
    trail_rgb:    Vec<Rgb>,
    trail_w:      usize,
    trail_h:      usize,

    scatter_timer: f64,   // time until next scatter event
    scatter_x:    f64,    // predator position (used during scatter)
    scatter_y:    f64,
    scatter_age:  f64,    // how long the current scatter has been active
    initialized:  bool,
}

impl BoidFlockMode {
    pub fn new(speed: f64, color: ColorProvider) -> Self {
        Self {
            speed, color,
            boids: Vec::new(),
            trail_bright: Vec::new(),
            trail_rgb: Vec::new(),
            trail_w: 0, trail_h: 0,
            scatter_timer: 18.0,
            scatter_x: 0.0, scatter_y: 0.0, scatter_age: 0.0,
            initialized: false,
        }
    }

    fn init(&mut self, pw: usize, ph: usize) {
        self.trail_w = pw;
        self.trail_h = ph;
        self.trail_bright = vec![0.0; pw * ph];
        self.trail_rgb    = vec![(0, 0, 0); pw * ph];

        let mut rng = rand::rng();
        // Spawn boids clustered near the centre
        let cx = pw as f64 / 2.0;
        let cy = ph as f64 / 2.0;
        self.boids = (0..NUM_BOIDS).map(|_| {
            let angle = rng.random_range(0.0..std::f64::consts::TAU);
            let spd   = rng.random_range(MIN_SPEED..MAX_SPEED) * self.speed;
            let r     = rng.random_range(0.0..20.0);
            Boid {
                x:  cx + angle.cos() * r,
                y:  cy + angle.sin() * r * 0.5,
                vx: angle.cos() * spd,
                vy: angle.sin() * spd,
            }
        }).collect();

        let mut rng = rand::rng();
        self.scatter_timer = rng.random_range(14.0..25.0) / self.speed.max(0.5);
        self.initialized = true;
    }

    /// Derive a colour for a boid with velocity (vx, vy).
    /// Maps heading angle to hue, then applies the ColorProvider tint.
    fn boid_color(&self, vx: f64, vy: f64, t: f64, px: f64) -> Rgb {
        // Heading angle 0..2π → hue 0..1
        let angle = vy.atan2(vx).rem_euclid(std::f64::consts::TAU);
        let hue   = angle / std::f64::consts::TAU;

        // Hue → RGB (three-phase cosine)
        let r = (0.5 + 0.5 * (hue * std::f64::consts::TAU                ).cos() * 255.0) as u8;
        let g = (0.5 + 0.5 * (hue * std::f64::consts::TAU + 2.094        ).cos() * 255.0) as u8;
        let b = (0.5 + 0.5 * (hue * std::f64::consts::TAU + 4.189        ).cos() * 255.0) as u8;
        let base: Rgb = (r, g, b);

        // Apply vibe tint
        match self.color.mode {
            ColorMode::Matrix => {
                let luma = (r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000;
                (0, (luma as f64 * 1.2).min(255.0) as u8, 0)
            }
            ColorMode::Ocean  => {
                let factor = ((t * self.color.speed + px * 0.05).sin() * 0.5 + 0.5) as f64;
                lerp((0, 20, 100), (64, 224, 208), factor)
            }
            ColorMode::Sunset => {
                let factor = ((t * self.color.speed + px * 0.05).sin() * 0.5 + 0.5) as f64;
                lerp((100, 60, 40), (255, 120, 80), factor)
            }
            _ => base,
        }
    }
}

fn lerp(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let c = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * t) as u8;
    (c(a.0,b.0), c(a.1,b.1), c(a.2,b.2))
}

impl Mode for BoidFlockMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t: f64) {
        let pw = width  as usize;
        let ph = height as usize * 2;

        if !self.initialized || self.trail_w != pw || self.trail_h != ph {
            self.init(pw, ph);
        }

        let dt   = dt * self.speed;
        let mut rng = rand::rng();

        // ── Scatter event timer ───────────────────────────────────────────────
        self.scatter_timer -= dt;
        self.scatter_age    = (self.scatter_age - dt).max(0.0);
        if self.scatter_timer <= 0.0 {
            self.scatter_x   = rng.random_range(pw as f64 * 0.2..pw as f64 * 0.8);
            self.scatter_y   = rng.random_range(ph as f64 * 0.2..ph as f64 * 0.8);
            self.scatter_age = 2.5;
            self.scatter_timer = rng.random_range(14.0..30.0) / self.speed.max(0.5);
        }

        // ── Boid physics ──────────────────────────────────────────────────────
        // Pre-snapshot positions to avoid using updated values mid-step.
        let snapshot: Vec<(f64, f64, f64, f64)> =
            self.boids.iter().map(|b| (b.x, b.y, b.vx, b.vy)).collect();

        for (i, b) in self.boids.iter_mut().enumerate() {
            let (bx, by, _bvx, _bvy) = snapshot[i];

            let mut sep_x = 0.0_f64; let mut sep_y = 0.0_f64;
            let mut ali_x = 0.0_f64; let mut ali_y = 0.0_f64;
            let mut coh_x = 0.0_f64; let mut coh_y = 0.0_f64;
            let mut neighbours = 0usize;

            for (j, &(ox, oy, ovx, ovy)) in snapshot.iter().enumerate() {
                if i == j { continue; }
                let dx = ox - bx;
                let dy = oy - by;
                let d2 = dx * dx + dy * dy;
                if d2 > PERCEPTION * PERCEPTION { continue; }
                let d = d2.sqrt().max(0.001);
                neighbours += 1;
                // Separation: repel from very-close neighbours
                if d < SEP_RADIUS {
                    sep_x -= dx / d;
                    sep_y -= dy / d;
                }
                // Alignment: average velocity of neighbours
                ali_x += ovx; ali_y += ovy;
                // Cohesion: move toward average position
                coh_x += ox;  coh_y += oy;
            }

            let mut ax = 0.0_f64;
            let mut ay = 0.0_f64;

            if neighbours > 0 {
                let n = neighbours as f64;
                // Normalize separation
                let sl = (sep_x * sep_x + sep_y * sep_y).sqrt().max(0.001);
                ax += (sep_x / sl) * SEP_WEIGHT;
                ay += (sep_y / sl) * SEP_WEIGHT;
                // Normalize alignment
                let al = (ali_x * ali_x + ali_y * ali_y).sqrt().max(0.001);
                ax += (ali_x / al) * ALIGN_WEIGHT;
                ay += (ali_y / al) * ALIGN_WEIGHT;
                // Cohesion toward centroid
                let cdx = coh_x / n - bx;
                let cdy = coh_y / n - by;
                let cl = (cdx * cdx + cdy * cdy).sqrt().max(0.001);
                ax += (cdx / cl) * COHESION_WEIGHT;
                ay += (cdy / cl) * COHESION_WEIGHT;
            }

            // Random jitter to prevent locking into perfect circles
            let noise_ang = rng.random_range(0.0..std::f64::consts::TAU);
            ax += noise_ang.cos() * NOISE_WEIGHT;
            ay += noise_ang.sin() * NOISE_WEIGHT;

            // Scatter force from predator
            if self.scatter_age > 0.0 {
                let sdx = bx - self.scatter_x;
                let sdy = by - self.scatter_y;
                let sd  = (sdx * sdx + sdy * sdy).sqrt().max(0.001);
                let force = (self.scatter_age / 2.5) * 6.0;
                ax += (sdx / sd) * force;
                ay += (sdy / sd) * force;
            }

            // Integrate velocity and clamp speed
            b.vx += ax * dt * 40.0;
            b.vy += ay * dt * 40.0;
            let spd = (b.vx * b.vx + b.vy * b.vy).sqrt().max(0.001);
            let max = MAX_SPEED * self.speed;
            let min = MIN_SPEED * self.speed;
            if spd > max { b.vx = b.vx / spd * max; b.vy = b.vy / spd * max; }
            if spd < min { b.vx = b.vx / spd * min; b.vy = b.vy / spd * min; }

            b.x += b.vx * dt;
            b.y += b.vy * dt;

            // Wrap around edges
            b.x = b.x.rem_euclid(pw as f64);
            b.y = b.y.rem_euclid(ph as f64);
        }

        // ── Trail decay ───────────────────────────────────────────────────────
        let decay = TRAIL_DECAY * dt as f32;
        for v in &mut self.trail_bright { *v = (*v - decay).max(0.0); }
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let pw = width  as usize;
        let ph = height as usize * 2;

        // Clone the trail buffer and paint boids into it
        let mut bright = self.trail_bright.clone();
        let mut trgb   = self.trail_rgb.clone();

        for b in &self.boids {
            let px = b.x as usize;
            let py = b.y as usize;
            if px < pw && py < ph {
                let idx = py * pw + px;
                bright[idx] = 1.0;
                trgb[idx]   = self.boid_color(b.vx, b.vy, t_abs, b.x);
            }
        }

        // ── Compose pixels into half-block frame ──────────────────────────────
        let w = width  as usize;
        let h = height as usize;

        // Dark background colour shifts very slowly
        let bg_pulse = (t_abs * 0.06).sin() * 0.5 + 0.5;
        let bg: Rgb  = (
            (5.0 + bg_pulse * 4.0) as u8,
            (6.0 + bg_pulse * 3.0) as u8,
            (18.0 + bg_pulse * 8.0) as u8,
        );

        let mut out = String::with_capacity(w * h * 42);

        for row in 0..h {
            for col in 0..w {
                let ui = row * 2 * pw + col;       // upper pixel
                let li = (row * 2 + 1) * pw + col; // lower pixel

                let (ur, ug, ub) = if ui < bright.len() && bright[ui] > 0.01 {
                    let b = bright[ui].clamp(0.0, 1.0);
                    let (r, g, bv) = trgb[ui];
                    ((r as f64 * b as f64 + bg.0 as f64 * (1.0 - b as f64)) as u8,
                     (g as f64 * b as f64 + bg.1 as f64 * (1.0 - b as f64)) as u8,
                     (bv as f64 * b as f64 + bg.2 as f64 * (1.0 - b as f64)) as u8)
                } else { bg };

                let (lr, lg, lb) = if li < bright.len() && bright[li] > 0.01 {
                    let b = bright[li].clamp(0.0, 1.0);
                    let (r, g, bv) = trgb[li];
                    ((r as f64 * b as f64 + bg.0 as f64 * (1.0 - b as f64)) as u8,
                     (g as f64 * b as f64 + bg.1 as f64 * (1.0 - b as f64)) as u8,
                     (bv as f64 * b as f64 + bg.2 as f64 * (1.0 - b as f64)) as u8)
                } else { bg };

                out.push_str(&format!(
                    "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m\u{2580}",
                    ur, ug, ub, lr, lg, lb,
                ));
            }
            out.push_str(RESET);
            if row < h - 1 { out.push('\n'); }
        }

        // Scatter-event warning flash
        if self.scatter_age > 0.0 {
            let alpha = (self.scatter_age / 2.5).clamp(0.0, 1.0);
            let v     = (alpha * 255.0) as u8;
            let sx    = (self.scatter_x as usize).clamp(1, w) / 2 + 1;
            let sy    = (self.scatter_y as usize).clamp(1, ph) / 2 + 1;
            out.push_str(&format!(
                "\x1b[{};{}H\x1b[38;2;{};0;0m✦",
                sy, sx, v,
            ));
        }

        out.push_str(RESET);
        out
    }
}
