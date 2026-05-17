// ===================================================================
//  src/modes/train_journey.rs
// -------------------------------------------------------------------
//  Steam locomotive locked at ~28% from left, world scrolling past
//  through four parallax layers (sky, far mountains, near hills,
//  foreground tracks). Day/night cycle ~5.5 min. Smoke puffs from
//  the chimney, wheels rotate, sun/moon arc overhead.
//
//  impl ModeDescriptor for TrainJourneyMode {
//      const ID:   &'static str = "train_journey";
//      const NAME: &'static str = "Train Journey";
//      const DESC: &'static str = "Scenic steam locomotive ride";
//      const FPS:  u32          = 50;
//  }
// ===================================================================

use crate::ansi::RESET;
use crate::color::ColorProvider;
use crate::mode_base::Mode;
use rand::RngExt;

type Rgb = (u8, u8, u8);

// ── Colour math ───────────────────────────────────────────────────────────────

fn lerp(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let c = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * t) as u8;
    (c(a.0, b.0), c(a.1, b.1), c(a.2, b.2))
}

fn blend(bg: Rgb, fg: Rgb, alpha: f64) -> Rgb { lerp(bg, fg, alpha) }

fn cycle4(t: f64, dawn: Rgb, day: Rgb, dusk: Rgb, night: Rgb) -> Rgb {
    match t {
        x if x < 0.25 => lerp(dawn,  day,   x          / 0.25),
        x if x < 0.50 => lerp(day,   dusk,  (x - 0.25) / 0.25),
        x if x < 0.75 => lerp(dusk,  night, (x - 0.50) / 0.25),
        x              => lerp(night, dawn,  (x - 0.75) / 0.25),
    }
}

fn sky_top(d: f64)    -> Rgb { cycle4(d, (75,40,90),   (15,90,195),  (110,38,28), (4,7,28))   }
fn sky_horiz(d: f64)  -> Rgb { cycle4(d, (220,130,75), (105,175,235),(240,88,32), (16,20,52)) }
fn far_mtn(d: f64)    -> Rgb { cycle4(d, (88,65,105),  (68,100,150), (118,52,52), (16,18,48)) }
fn near_hill(d: f64)  -> Rgb { cycle4(d, (38,68,48),   (32,93,42),   (52,46,26),  (9,20,11))  }
fn ground_col(d: f64) -> Rgb { cycle4(d, (48,75,28),   (52,118,26),  (63,70,18),  (11,24,9))  }
fn rail_col(d: f64)   -> Rgb { cycle4(d, (60,50,40),   (90,82,72),   (75,55,40),  (25,20,16)) }

fn star_alpha(d: f64) -> f64 {
    if      d < 0.08 { 1.0 - d / 0.08 }
    else if d < 0.42 { 0.0 }
    else if d < 0.50 { (d - 0.42) / 0.08 }
    else if d < 0.92 { 1.0 }
    else             { 1.0 - (d - 0.92) / 0.08 }
}

fn fbm(x: f64, seed: f64) -> f64 {
    0.40 * (x * 0.013 + seed          ).sin()
  + 0.25 * (x * 0.027 + seed * 1.73  ).sin()
  + 0.15 * (x * 0.061 + seed * 0.91  ).sin()
  + 0.10 * (x * 0.110 + seed * 2.31  ).sin()
  + 0.05 * (x * 0.220 + seed * 1.47  ).sin()
}

// ── Smoke puffs ───────────────────────────────────────────────────────────────

struct Puff {
    x: f64, y: f64,
    radius: f64,
    age: f64,
    drift_x: f64,
    drift_y: f64,
}

// ── Mode ──────────────────────────────────────────────────────────────────────

pub struct TrainJourneyMode {
    speed: f64,
    _color: ColorProvider,
    scroll: f64,
    wheel_phase: f64,
    puff_timer: f64,
    puffs: Vec<Puff>,
    far_seed: f64,
    near_seed: f64,
}

impl TrainJourneyMode {
    pub fn new(speed: f64, color: ColorProvider) -> Self {
        let mut rng = rand::rng();
        Self {
            speed,
            _color: color,
            scroll: 0.0,
            wheel_phase: 0.0,
            puff_timer: 0.0,
            puffs: Vec::new(),
            far_seed:  rng.random_range(0.0..100.0),
            near_seed: rng.random_range(0.0..100.0),
        }
    }
}

impl Mode for TrainJourneyMode {
    fn update(&mut self, dt: f64, _w: u16, _h: u16, _t: f64) {
        let dt = dt * self.speed;
        self.scroll      += 22.0 * dt;
        self.wheel_phase += 9.0  * dt;
        self.puff_timer  += dt;

        // Spawn smoke puffs at the chimney (~every 0.18s)
        if self.puff_timer >= 0.18 / self.speed.max(0.3) {
            self.puff_timer = 0.0;
            let mut rng = rand::rng();
            self.puffs.push(Puff {
                x: 0.0, y: 0.0,
                radius:  rng.random_range(1.5..2.5),
                age:     0.0,
                drift_x: rng.random_range(-1.5..0.5),
                drift_y: rng.random_range(-3.5..-2.0),
            });
        }

        for p in &mut self.puffs {
            p.age    += dt;
            p.x      += p.drift_x * dt;
            p.y      += p.drift_y * dt;
            p.radius += 1.4 * dt;
        }
        self.puffs.retain(|p| p.age < 2.5);
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w  = width  as usize;
        let h  = height as usize;
        let ph = h * 2; // pixel height via half-block

        let day_t    = (t_abs * self.speed * 0.003).rem_euclid(1.0);
        let horizon  = (ph as f64 * 0.62) as usize;
        let track_px = (ph as f64 * 0.85) as usize;

        let mut pix: Vec<Vec<Rgb>> = vec![vec![(0, 0, 0); w]; ph];

        // ── Sky ───────────────────────────────────────────────────
        for y in 0..horizon.min(ph) {
            let t   = y as f64 / horizon as f64;
            let col = lerp(sky_top(day_t), sky_horiz(day_t), t.powf(0.7));
            for x in 0..w { pix[y][x] = col; }
        }

        // ── Stars ─────────────────────────────────────────────────
        let sa = star_alpha(day_t);
        if sa > 0.01 {
            for i in 0..220usize {
                let sx = (i.wrapping_mul(2654435761) >> 3) % w.max(1);
                let sy = (i.wrapping_mul(2246822519) >> 3) % (horizon / 2).max(1);
                let twinkle = ((t_abs * 1.7 + i as f64 * 0.55).sin() + 1.0) * 0.5;
                let b = (twinkle * sa * 230.0) as u8;
                if b > 25 { pix[sy][sx] = (b, b, (b as f64 * 0.9) as u8); }
            }
        }

        // ── Sun / Moon ────────────────────────────────────────────
        let is_day = day_t < 0.5;
        let arc_t  = if is_day { day_t / 0.5 } else { (day_t - 0.5) / 0.5 };
        let sx = (arc_t * (w as f64 - 6.0) + 3.0) as i32;
        let sy = (horizon as f64 * (0.10 + (1.0 - (arc_t * std::f64::consts::PI).sin()) * 0.55)) as i32;
        let (body, glow, r): (Rgb, Rgb, i32) =
            if is_day { ((255, 245, 175), (255, 210, 70), 4) }
            else      { ((230, 230, 215), (170, 175, 195), 3) };
        for dy in -(r + 4)..=(r + 4) {
            for dx in -(r + 4)..=(r + 4) {
                let d2 = dx * dx + dy * dy;
                if d2 > r * r && d2 <= (r + 4) * (r + 4) {
                    let a  = 0.22 * (1.0 - (d2 as f64).sqrt() / (r as f64 + 4.0));
                    let px = sx + dx; let py = sy + dy;
                    if px >= 0 && px < w as i32 && py >= 0 && py < horizon as i32 {
                        pix[py as usize][px as usize] =
                            blend(pix[py as usize][px as usize], glow, a);
                    }
                }
            }
        }
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r {
                    let px = sx + dx; let py = sy + dy;
                    if px >= 0 && px < w as i32 && py >= 0 && py < horizon as i32 {
                        pix[py as usize][px as usize] = body;
                    }
                }
            }
        }

        // ── Far mountains ─────────────────────────────────────────
        let far_col    = far_mtn(day_t);
        let far_offset = self.scroll * 0.10;
        for x in 0..w {
            let tx  = x as f64 + far_offset;
            let hf  = (fbm(tx, self.far_seed) + 1.0) * 0.5;
            let top = (horizon as f64 * (0.55 + hf * 0.35)) as usize;
            for y in top.min(horizon)..horizon {
                let d = (y - top.min(horizon)) as f64 / (horizon - top.min(horizon)).max(1) as f64;
                pix[y][x] = blend(pix[y][x], lerp(far_col, (far_col.0/2, far_col.1/2, far_col.2/2), d * 0.4), 0.92);
            }
        }

        // ── Near hills ────────────────────────────────────────────
        let near_col    = near_hill(day_t);
        let near_offset = self.scroll * 0.32;
        for x in 0..w {
            let tx  = x as f64 + near_offset;
            let hf  = (fbm(tx, self.near_seed) + 1.0) * 0.5;
            let top = (ph as f64 * (0.55 + hf * 0.18)) as usize;
            for y in top.min(ph)..ph {
                let d = (y - top.min(ph)) as f64 / (ph - top.min(ph)).max(1) as f64;
                pix[y][x] = blend(pix[y][x], lerp(near_col, (near_col.0/2, near_col.1/2, near_col.2/2), d * 0.45), 0.96);
            }
        }

        // ── Ground ────────────────────────────────────────────────
        let g_col = ground_col(day_t);
        for y in track_px.saturating_sub(4)..ph {
            for x in 0..w {
                let depth = (y - track_px.saturating_sub(4)) as f64 / 8.0;
                pix[y][x] = lerp(g_col, (g_col.0/2, g_col.1/2, g_col.2/2), depth.min(1.0) * 0.4);
            }
        }

        // ── Tracks ────────────────────────────────────────────────
        let r_col       = rail_col(day_t);
        let track_offset = self.scroll;
        for rail_y in [track_px.saturating_sub(1), track_px + 1] {
            if rail_y < ph { for x in 0..w { pix[rail_y][x] = r_col; } }
        }
        let tie_spacing = 3.0_f64;
        let tie_offset  = track_offset.rem_euclid(tie_spacing);
        let mut tx = -tie_offset;
        while tx < w as f64 {
            let xi = tx.round() as i32;
            for dx in 0..2i32 {
                let cx = xi + dx;
                if cx >= 0 && cx < w as i32 {
                    for dy in 0..3i32 {
                        let cy = track_px as i32 + dy - 1;
                        if cy >= 0 && cy < ph as i32 {
                            pix[cy as usize][cx as usize] = (60, 45, 30);
                        }
                    }
                }
            }
            tx += tie_spacing;
        }

        // ── Locomotive ────────────────────────────────────────────
        // Train sits at ~28% from left, front (cowcatcher) faces RIGHT.
        let train_col_x = (w as f64 * 0.28) as i32;
        let train_base_py = track_px as i32 - 1;
        draw_locomotive(&mut pix, w, ph, train_col_x, train_base_py, self.wheel_phase, day_t);

        // ── Smoke puffs ───────────────────────────────────────────
        // Chimney is at offset +17 from the left edge of the locomotive.
        let chimney_x = train_col_x as f64 + 17.5;
        let chimney_y = (train_base_py - 9) as f64;
        for p in &self.puffs {
            let pcx = chimney_x + p.x;
            let pcy = chimney_y + p.y;
            let age_t = (p.age / 2.5).clamp(0.0, 1.0);
            let alpha = (1.0 - age_t) * 0.7;
            let col   = lerp((220, 220, 220), sky_horiz(day_t), age_t * 0.5);
            let r2    = p.radius;
            for dy in -(r2 as i32 + 1)..=(r2 as i32 + 1) {
                for dx in -(r2 as i32 + 1)..=(r2 as i32 + 1) {
                    let fx = dx as f64 / r2;
                    let fy = (dy as f64 / r2) * 0.5;
                    let d  = fx * fx + fy * fy;
                    if d <= 1.0 {
                        let pa = (1.0 - d.sqrt()).powi(2) * alpha;
                        let px = pcx + dx as f64; let py = pcy + dy as f64;
                        if px >= 0.0 && px < w as f64 && py >= 0.0 && py < ph as f64 {
                            pix[py as usize][px as usize] =
                                blend(pix[py as usize][px as usize], col, pa);
                        }
                    }
                }
            }
        }

        // ── Compose half-blocks ───────────────────────────────────
        let mut out = String::with_capacity(w * h * 42);
        for row in 0..h {
            for col in 0..w {
                let (ur, ug, ub) = pix[row * 2][col];
                let (lr, lg, lb) = if row * 2 + 1 < ph { pix[row * 2 + 1][col] } else { (0, 0, 0) };
                out.push_str(&format!(
                    "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m\u{2580}",
                    ur, ug, ub, lr, lg, lb,
                ));
            }
            out.push_str(RESET);
            if row < h - 1 { out.push('\n'); }
        }
        out
    }
}

// ── Locomotive renderer ───────────────────────────────────────────────────────
//
// `bx` = leftmost column of the entire train.
// `by` = bottom pixel row (where wheels sit on the rails).
//
// Layout, left to right (train faces RIGHT):
//
//   TENDER │ CAB │─── BOILER ───│ CHIMNEY │ COWCATCHER
//   bx+0   │bx+5 │  bx+10..20  │  bx+17  │   bx+21
//
// This matches the direction of travel — the cowcatcher leads, the
// coal tender is at the back (left).

fn draw_locomotive(
    pix: &mut [Vec<Rgb>],
    w: usize, ph: usize,
    bx: i32, by: i32,
    wheel_phase: f64, day_t: f64,
) {
    let body  = lerp((130, 35, 35), (60, 18, 18), if day_t > 0.5 { 0.6 } else { 0.0 });
    let trim  = (245, 215, 90);
    let dark  = (28, 18, 12);
    let metal = (90, 90, 95);
    let win   = lerp((255, 230, 110), (95, 80, 100), if day_t > 0.5 { 0.7 } else { 0.0 });

    // Safe pixel setter — bounds-checked.
    let put = |buf: &mut [Vec<Rgb>], x: i32, y: i32, c: Rgb| {
        if x >= 0 && x < w as i32 && y >= 0 && y < ph as i32 {
            buf[y as usize][x as usize] = c;
        }
    };

    // ── Coal Tender (back of train, leftmost) ─────────────────────────────────
    for dy in 0..4 {
        for dx in 0..6 {
            let col = if dy == 0 { lerp(body, dark, 0.3) } else { body };
            put(pix, bx + dx, by - 3 + dy, col);
        }
    }
    // Coal pile on top of tender
    for dx in 0..6 { put(pix, bx + dx, by - 4, (40, 35, 35)); }

    // ── Cab (engineer's cabin, just ahead of tender) ──────────────────────────
    for dy in 0..7 {
        for dx in 0..6 {
            let col = if dy == 0 || dy == 6 || dx == 0 || dx == 5 {
                lerp(body, dark, 0.3)
            } else { body };
            put(pix, bx + 5 + dx, by - 6 + dy, col);
        }
    }
    // Rear window (faces backward — left side of cab)
    for dy in 0..2 {
        for dx in 0..2 { put(pix, bx + 6 + dx, by - 5 + dy, win); }
    }
    // Roof overhang
    for dx in -1..=6i32 { put(pix, bx + 5 + dx, by - 7, dark); }

    // ── Boiler (main cylinder, runs the majority of the locomotive) ───────────
    for dy in 0..5 {
        for dx in 0..11 {
            let col = if dy == 0 || dy == 4 { lerp(body, dark, 0.4) } else { body };
            put(pix, bx + 10 + dx, by - 4 + dy, col);
        }
    }
    // Decorative boiler bands
    for &dx in &[3i32, 7i32] {
        for dy in -4..=0 { put(pix, bx + 10 + dx, by + dy, trim); }
    }
    // Front face of boiler (right-facing)
    for dy in -4..=0 { put(pix, bx + 21, by + dy, dark); }

    // ── Steam dome (small bump on top of boiler) ──────────────────────────────
    put(pix, bx + 14, by - 5, trim);
    put(pix, bx + 15, by - 5, trim);

    // ── Chimney / smokestack (tall tube near the front of the boiler) ─────────
    for dy in 0..5 {
        put(pix, bx + 17, by - 5 - dy, dark);
        put(pix, bx + 18, by - 5 - dy, dark);
    }
    // Chimney top widens slightly (flare)
    put(pix, bx + 16, by - 9, dark);
    put(pix, bx + 19, by - 9, dark);

    // ── Headlight (on the front face) ─────────────────────────────────────────
    put(pix, bx + 22, by - 2, lerp(trim, (255, 255, 220), 0.5));

    // ── Cowcatcher (V-shape at the very front) ────────────────────────────────
    for dy in 0..3i32 {
        for dx in 0..(3 - dy) {
            put(pix, bx + 22 + dx, by + dy - 1, lerp(metal, dark, 0.3));
        }
    }

    // ── Wheels (tender, cab, mid-boiler) ──────────────────────────────────────
    let wheel_centers = [bx + 2, bx + 8, bx + 15];
    for &wcx in &wheel_centers {
        let wcy = by + 1;
        for dy in -1..=1i32 {
            for dx in -1..=1i32 {
                let d2 = dx * dx + dy * dy;
                if d2 == 1 || (d2 == 2 && dx.abs() + dy.abs() == 2) {
                    put(pix, wcx + dx, wcy + dy, dark);
                }
            }
        }
        put(pix, wcx, wcy, metal);
        // Rotating spoke
        let angle = wheel_phase + (wcx as f64) * 0.4;
        let sx = (angle.cos() * 1.0).round() as i32;
        let sy = (angle.sin() * 1.0).round() as i32;
        put(pix, wcx + sx, wcy + sy, trim);
    }
}