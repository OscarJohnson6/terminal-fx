// ===== src/modes/landscape.rs =====
//
// A layered landscape rendered with Unicode UPPER HALF BLOCK (▀) characters
// and TrueColor ANSI sequences. Each terminal cell represents TWO pixel rows:
//
//   ┌────────────────────┐
//   │  ▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀ │  ← Each '▀' holds upper pixel (fg) + lower pixel (bg)
//   └────────────────────┘
//
// This gives double the vertical resolution of plain character rendering —
// a 80×24 terminal becomes an 80×48 "pixel" canvas.
//
// Scene layers (back → front):
//   Sky gradient → Stars → Sun/Moon → Far mountains → Near mountains →
//   Ground + water → Fog → Clouds → Trees → Birds
//
// The scene slowly cycles: dawn → day → dusk → night (~5 min per cycle).
use crate::color::ColorProvider;
use crate::mode_base::Mode;
use rand::{RngExt};
use std::f64::consts::PI;

// ── Type aliases ──────────────────────────────────────────────────────────────
type Rgb = (u8, u8, u8);
type Pix = Vec<Vec<Rgb>>; // Pix[pixel_y][x], y=0 is top

// ── Colour math ───────────────────────────────────────────────────────────────

fn lerp(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let c = |av: u8, bv: u8| (av as f64 + (bv as f64 - av as f64) * t) as u8;
    (c(a.0, b.0), c(a.1, b.1), c(a.2, b.2))
}

fn blend(bg: Rgb, fg: Rgb, alpha: f64) -> Rgb { lerp(bg, fg, alpha) }

fn darken(c: Rgb, f: f64) -> Rgb {
    let s = (1.0 - f).clamp(0.0, 1.0);
    ((c.0 as f64 * s) as u8, (c.1 as f64 * s) as u8, (c.2 as f64 * s) as u8)
}

fn brighten(c: Rgb, n: i32) -> Rgb {
    let b = |v: u8| (v as i32 + n).clamp(0, 255) as u8;
    (b(c.0), b(c.1), b(c.2))
}

/// Smoothly cycle through 4 colours over t ∈ [0, 1).
/// Phase: 0.00 = dawn · 0.25 = day · 0.50 = dusk · 0.75 = night
fn cycle4(t: f64, dawn: Rgb, day: Rgb, dusk: Rgb, night: Rgb) -> Rgb {
    match t {
        t if t < 0.25 => lerp(dawn,  day,   t           / 0.25),
        t if t < 0.50 => lerp(day,   dusk,  (t - 0.25)  / 0.25),
        t if t < 0.75 => lerp(dusk,  night, (t - 0.50)  / 0.25),
        t              => lerp(night, dawn,  (t - 0.75)  / 0.25),
    }
}

// ── Sky / scene colour palette ────────────────────────────────────────────────
fn sky_top(d: f64)     -> Rgb { cycle4(d, (75,40,90),   (15,90,195),  (110,38,28), (4,7,28))   }
fn sky_horiz(d: f64)   -> Rgb { cycle4(d, (220,130,75), (105,175,235),(240,88,32), (16,20,52)) }
fn mtn_far(d: f64)     -> Rgb { cycle4(d, (88,65,105),  (68,100,150), (118,52,52), (16,18,48)) }
fn mtn_near(d: f64)    -> Rgb { cycle4(d, (38,68,48),   (32,83,42),   (52,46,26),  (9,20,11))  }
fn col_grass(d: f64)   -> Rgb { cycle4(d, (44,88,34),   (52,118,26),  (63,76,17),  (11,26,7))  }
fn col_earth(d: f64)   -> Rgb { cycle4(d, (66,46,26),   (78,57,32),   (66,43,23),  (21,15,9))  }
fn col_water(d: f64)   -> Rgb { cycle4(d, (48,78,128),  (33,108,188), (78,68,118), (9,18,58))  }
fn col_cloud(d: f64)   -> Rgb { cycle4(d, (238,198,168),(244,244,252),(255,173,98),(68,73,98))  }
fn col_trunk(d: f64)   -> Rgb { cycle4(d, (58,38,18),   (68,48,24),   (63,40,18),  (18,13,7))  }
fn col_leaf(d: f64)    -> Rgb { cycle4(d, (28,63,23),   (38,88,18),   (48,68,13),  (7,20,4))   }

fn star_vis(d: f64) -> f64 {
    // Full stars at night, fade during dawn and dusk transitions
    if      d < 0.08 { 1.0 - d / 0.08 }
    else if d < 0.42 { 0.0 }
    else if d < 0.50 { (d - 0.42) / 0.08 }
    else if d < 0.92 { 1.0 }
    else             { 1.0 - (d - 0.92) / 0.08 }
}

// ── Procedural terrain ────────────────────────────────────────────────────────

/// Fractal-like height via summed sines. Returns roughly −1..1.
/// Using sines instead of a proper noise library keeps zero dependencies.
fn fbm(x: f64, seed: f64) -> f64 {
    0.40 * (x * 0.011 + seed         ).sin()
  + 0.25 * (x * 0.024 + seed * 1.73  ).sin()
  + 0.15 * (x * 0.051 + seed * 0.91  ).sin()
  + 0.10 * (x * 0.097 + seed * 2.31  ).sin()
  + 0.05 * (x * 0.190 + seed * 1.47  ).sin()
  + 0.05 * (x * 0.370 + seed * 0.53  ).sin()
}

/// Smooth scrolling noise for wind/grass animation.
fn wind_noise(x: f64, t: f64) -> f64 {
    0.50 * (x * 0.14 + t * 2.1).sin()
  + 0.30 * (x * 0.29 + t * 1.6).sin()
  + 0.20 * (x * 0.61 + t * 3.3).sin()
}

// ── Scene object types ────────────────────────────────────────────────────────

struct Cloud {
    x:         f64,
    y_frac:    f64, // 0..1 fraction of sky height
    rw:        f64, // horizontal radius in columns
    rh:        f64, // vertical radius in pixel rows
    speed_mul: f64,
}

struct Bird {
    x:       f64,
    y_frac:  f64,
    speed:   f64,
    wing:    f64, // wing phase in radians
}

struct Tree {
    x_frac:   f64, // horizontal position as fraction of width
    trunk_px: f64, // trunk height in pixels
    canopy_r: f64, // canopy radius in pixels
}

// ── The mode ──────────────────────────────────────────────────────────────────

pub struct LandscapeMode {
    speed_factor: f64,
    color_provider: ColorProvider,
    wind_offset:  f64,
    clouds:       Vec<Cloud>,
    birds:        Vec<Bird>,
    trees:        Vec<Tree>,
    far_seed:     f64,
    near_seed:    f64,
    hill_seed:    f64,
}

impl LandscapeMode {
    pub fn new(speed_factor: f64, color_provider: ColorProvider) -> Self {
        let mut rng = rand::rng();

        let clouds = (0..6).map(|_| Cloud {
            x:         rng.random_range(0.0..300.0),
            y_frac:    rng.random_range(0.04..0.40),
            rw:        rng.random_range(18.0..48.0),
            rh:        rng.random_range(3.5..9.0),
            speed_mul: rng.random_range(0.3..1.2),
        }).collect();

        let bird_count = rng.random_range(4..9);
        let birds = (0..bird_count).map(|_| Bird {
            x:      rng.random_range(-60.0..300.0),
            y_frac: rng.random_range(0.07..0.48),
            speed:  rng.random_range(7.0..19.0) * speed_factor,
            wing:   rng.random_range(0.0..std::f64::consts::TAU),
        }).collect();

        // Trees placed at roughly evenly-spaced horizontal positions
        let trees = (0..5).map(|i| Tree {
            x_frac:   (i as f64 + rng.random_range(0.1..0.8)) / 5.0,
            trunk_px: rng.random_range(8.0..20.0),
            canopy_r: rng.random_range(7.0..15.0),
        }).collect();

        Self {
            speed_factor,
            color_provider,
            wind_offset: 0.0,
            clouds,
            birds,
            trees,
            far_seed:  rng.random_range(0.0..100.0),
            near_seed: rng.random_range(0.0..100.0),
            hill_seed: rng.random_range(0.0..100.0),
        }
    }
}

impl Mode for LandscapeMode {
    fn update(&mut self, dt: f64, width: u16, _height: u16, t_abs: f64) {
        let w = width as f64;
        // Gust factor: wind slowly strengthens and eases
        let gust = 0.65 + 0.60 * (t_abs * 0.27).sin();

        self.wind_offset += 14.0 * self.speed_factor * gust * dt;

        for cloud in &mut self.clouds {
            cloud.x += 5.0 * self.speed_factor * cloud.speed_mul * gust * dt;
            if cloud.x > w + cloud.rw * 2.0 {
                cloud.x = -cloud.rw * 2.0;
            }
        }

        for bird in &mut self.birds {
            bird.x    += bird.speed * dt;
            // Wing flap speeds up/slows down slightly for organic feel
            bird.wing += dt * (4.8 + 1.5 * (bird.x * 0.07).sin());
            if bird.x > w + 20.0 {
                bird.x = -20.0;
                let mut rng = rand::rng();
                bird.y_frac = rng.random_range(0.07..0.48);
                bird.speed  = rng.random_range(7.0..19.0) * self.speed_factor;
            }
        }
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w  = width  as usize;
        let h  = height as usize;
        let ph = h * 2; // pixel height — doubled by half-block trick

        // Initialise pixel buffer with black
        let mut pix: Pix = vec![vec![(0, 0, 0); w]; ph];

        // day_t: full cycle over ~5.5 min at speed_factor=1
        let day_t = (t_abs * self.speed_factor * 0.003).rem_euclid(1.0);
        let gust  = 0.65 + 0.60 * (t_abs * 0.27).sin();

        // sky_px: the pixel row where sky ends and mountains/ground begin
        let sky_px = (ph as f64 * 0.62) as usize;

        // ── Layer 1: Sky gradient ──────────────────────────────────────────
        for y in 0..sky_px.min(ph) {
            let t   = y as f64 / sky_px as f64;
            let col = lerp(sky_top(day_t), sky_horiz(day_t), t.powf(0.65));
            for x in 0..w { pix[y][x] = col; }
        }

        // ── Layer 2: Stars (night only) ────────────────────────────────────
        let sv = star_vis(day_t);
        if sv > 0.01 {
            // Deterministic star positions via integer hashing — no stored state needed.
            for i in 0..280usize {
                let sx = (i.wrapping_mul(2654435761) >> 3) % w.max(1);
                let sy = (i.wrapping_mul(2246822519) >> 3) % (sky_px / 2).max(1);
                let twinkle = ((t_abs * 1.9 + i as f64 * 0.61).sin() + 1.0) * 0.5;
                let b = (twinkle * sv * 238.0) as u8;
                if b > 18 {
                    pix[sy][sx] = (b, b, (b as f64 * 0.87) as u8);
                }
            }
        }

        // ── Layer 3: Sun / Moon ────────────────────────────────────────────
        paint_sun_moon(&mut pix, w, sky_px, day_t);

        // ── Layer 4: Far mountains (very slow parallax) ────────────────────
        let far_base = sky_px.saturating_sub(sky_px / 3);
        let far_col  = mtn_far(day_t);
        for x in 0..w {
            let tx     = x as f64 + self.wind_offset * 0.08;
            let hf     = (fbm(tx, self.far_seed) + 1.0) / 2.0;
            let top    = (far_base as f64 + hf * (sky_px - far_base) as f64 * 0.58) as usize;
            for y in top.min(sky_px)..sky_px {
                let d   = (y - top.min(sky_px)) as f64 / (sky_px - top.min(sky_px)).max(1) as f64;
                pix[y][x] = blend(pix[y][x], darken(far_col, d * 0.42), 0.93);
            }
        }

        // ── Layer 5: Near mountains ────────────────────────────────────────
        let near_col = mtn_near(day_t);
        for x in 0..w {
            let tx  = x as f64 + self.wind_offset * 0.22;
            let hf  = (fbm(tx, self.near_seed) + 1.0) / 2.0;
            let top = ((ph as f64) * (0.50 + hf * 0.27)) as usize;
            for y in top.min(ph)..ph {
                let d   = (y - top.min(ph)) as f64 / (ph - top.min(ph)).max(1) as f64;
                pix[y][x] = blend(pix[y][x], darken(near_col, d * 0.58), 0.97);
            }
        }

        // ── Layer 6: Ground + Water ────────────────────────────────────────
        let g_col = col_grass(day_t);
        let e_col = col_earth(day_t);
        let w_col = col_water(day_t);
        let water_level = (ph as f64 * 0.82) as usize; // water fills valleys below this line

        for x in 0..w {
            let tx          = x as f64 + self.wind_offset * 0.55;
            let hf          = (fbm(tx, self.hill_seed) + 1.0) / 2.0;
            let ground_row  = ((ph as f64) * (0.62 + hf * 0.14)) as usize;
            let ground_row  = ground_row.min(ph.saturating_sub(2));
            let in_valley   = ground_row > water_level;

            if in_valley {
                // Water surface: horizontal shimmer
                for y in water_level..ground_row.min(ph) {
                    let shimmer = 0.13 * (x as f64 * 0.35 + t_abs * 3.1 + y as f64 * 0.12).sin();
                    let depth   = (y - water_level) as f64 / (ground_row - water_level).max(1) as f64;
                    let col     = lerp(brighten(w_col, (shimmer * 40.0) as i32), darken(w_col, 0.4), depth);
                    pix[y][x]   = col;
                }
                for y in ground_row.min(ph)..ph { pix[y][x] = e_col; }
            } else {
                let ripple = wind_noise(x as f64, t_abs * self.speed_factor * gust);
                for y in ground_row..ph {
                    let d   = (y - ground_row) as f64 / (ph - ground_row).max(1) as f64;
                    let col = lerp(g_col, e_col, d.powf(0.55));
                    // Wind ripple brightens the very top of the grass
                    pix[y][x] = if d < 0.12 { brighten(col, (ripple * 20.0) as i32) } else { col };
                }
            }
        }

        // ── Layer 7: Valley fog ────────────────────────────────────────────
        // let fv = fog_vis(day_t);
        // if fv > 0.02 {
        //     let f_col = col_fog(day_t);
        //     for x in 0..w {
        //         let tx         = x as f64 + self.wind_offset * 0.55;
        //         let hf         = (fbm(tx, self.hill_seed) + 1.0) / 2.0;
        //         let ground_row = ((ph as f64) * (0.62 + hf * 0.14)) as usize;
        //         for dy in 0..14usize {
        //             let fy = ground_row + dy;
        //             if fy >= ph { break; }
        //             let density = (1.0 - dy as f64 / 14.0).powi(2) * fv;
        //             let drift   = 0.28 * (x as f64 * 0.045 + t_abs * 0.38).sin();
        //             pix[fy][x]  = blend(pix[fy][x], f_col, (density + drift).clamp(0.0, 0.75));
        //         }
        //     }
        // }

        // ── Layer 8: Clouds ────────────────────────────────────────────────
        for cloud in &self.clouds {
            paint_cloud(&mut pix, w, sky_px, cloud, day_t);
        }

        // ── Layer 9: Trees ────────────────────────────────────────────────
        // Canopy sways left/right with the wind
        let lean = wind_noise(0.0, t_abs * self.speed_factor) * 1.8;
        for tree in &self.trees {
            let col_x      = (tree.x_frac * w as f64) as usize;
            let tx         = col_x as f64 + self.wind_offset * 0.55;
            let hf         = (fbm(tx, self.hill_seed) + 1.0) / 2.0;
            let ground_row = ((ph as f64) * (0.62 + hf * 0.14)) as usize;
            let ground_row = ground_row.min(ph.saturating_sub(2));
            paint_tree(&mut pix, w, ph, col_x, ground_row, tree, day_t, lean);
        }

        // ── Layer 10: Birds ───────────────────────────────────────────────
        for bird in &self.birds {
            paint_bird(&mut pix, w, sky_px, bird);
        }

        // ── Compose into terminal string ───────────────────────────────────
        half_blocks(&pix, w, h, &self.color_provider, t_abs)
    }
}

// ── Drawing helpers ────────────────────────────────────────────────────────────

fn paint_sun_moon(pix: &mut Pix, w: usize, sky_h: usize, day_t: f64) {
    let is_day = day_t < 0.5;
    let arc_t  = if is_day { day_t / 0.5 } else { (day_t - 0.5) / 0.5 };

    // Horizontal sweep left→right, vertical arc peaks at arc_t = 0.5
    let sx = (arc_t * (w as f64 - 4.0) + 2.0) as i32;
    let sy = (sky_h as f64 * (0.07 + (1.0 - (arc_t * PI).sin()) * 0.52)) as i32;

    let (body, glow, r): (Rgb, Rgb, i32) = if is_day {
        ((255, 248, 180), (255, 215, 70), 4)
    } else {
        ((228, 228, 210), (175, 180, 198), 3)
    };

    // Soft glow halo
    for dy in -(r + 4)..=(r + 4) {
        for dx in -(r + 4)..=(r + 4) {
            let d2  = dx * dx + dy * dy;
            let max = (r + 4) * (r + 4);
            if d2 > r * r && d2 <= max {
                let a  = 0.22 * (1.0 - (d2 as f64).sqrt() / (r as f64 + 4.0));
                let px = sx + dx;
                let py = sy + dy;
                if px >= 0 && px < w as i32 && py >= 0 && py < sky_h as i32 {
                    pix[py as usize][px as usize] =
                        blend(pix[py as usize][px as usize], glow, a);
                }
            }
        }
    }
    // Solid disk
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r {
                let px = sx + dx;
                let py = sy + dy;
                if px >= 0 && px < w as i32 && py >= 0 && py < sky_h as i32 {
                    pix[py as usize][px as usize] = body;
                }
            }
        }
    }
}

fn paint_cloud(pix: &mut Pix, w: usize, sky_h: usize, cloud: &Cloud, day_t: f64) {
    let c_col = col_cloud(day_t);
    let cx    = cloud.x as i32;
    let cy    = (cloud.y_frac * sky_h as f64) as i32;
    let rw    = cloud.rw as i32;
    let rh    = cloud.rh as i32;

    // Main ellipse body
    for dy in -rh..=rh {
        for dx in -rw..=rw {
            let ex = dx as f64 / rw as f64;
            let ey = dy as f64 / rh as f64;
            let d  = ex * ex + ey * ey;
            if d <= 1.0 {
                let alpha = (1.0 - d).powf(0.45) * 0.83;
                let (px, py) = (cx + dx, cy + dy);
                if px >= 0 && px < w as i32 && py >= 0 && py < sky_h as i32 {
                    pix[py as usize][px as usize] =
                        blend(pix[py as usize][px as usize], c_col, alpha);
                }
            }
        }
    }

    // Three rounded bumps along the top — gives clouds their puffy silhouette
    for b in 0..3i32 {
        let bx = cx + (b - 1) * (rw / 2);
        let br = (rh * 3 / 2).max(2);
        for dy in -br..=0i32 {
            for dx in -br..=br {
                let ex = dx as f64 / br as f64;
                let ey = dy as f64 / br as f64;
                let d  = ex * ex + ey * ey;
                if d <= 1.0 {
                    let alpha = (1.0 - d).powf(0.45) * 0.72;
                    let (px, py) = (bx + dx, cy + dy);
                    if px >= 0 && px < w as i32 && py >= 0 && py < sky_h as i32 {
                        pix[py as usize][px as usize] =
                            blend(pix[py as usize][px as usize], c_col, alpha);
                    }
                }
            }
        }
    }
}

fn paint_bird(pix: &mut Pix, w: usize, sky_h: usize, bird: &Bird) {
    let bx  = bird.x as i32;
    let by  = (bird.y_frac * sky_h as f64) as i32;
    let col: Rgb = (22, 15, 10);

    // Wing flap: both wings angle up/down together, mimicking a V-shape in flight
    let wy = (bird.wing.sin() * 1.8) as i32;

    // Body
    sp(pix, w, sky_h, bx,     by,         col);
    // Left wing (2 segments)
    sp(pix, w, sky_h, bx - 1, by + wy,    col);
    sp(pix, w, sky_h, bx - 2, by + wy + 1,col);
    // Right wing
    sp(pix, w, sky_h, bx + 1, by + wy,    col);
    sp(pix, w, sky_h, bx + 2, by + wy + 1,col);
}

fn paint_tree(
    pix: &mut Pix, w: usize, ph: usize,
    x: usize, ground: usize,
    tree: &Tree, day_t: f64, lean: f64,
) {
    let t_col  = col_trunk(day_t);
    let l_col  = col_leaf(day_t);
    let l_dark = darken(l_col, 0.35);

    // Trunk: 3 columns wide, grows upward from ground
    let th = tree.trunk_px as usize;
    for dy in 0..th {
        let ty = ground.saturating_sub(dy);
        if ty >= ph { continue; }
        for tx in x.saturating_sub(1)..=(x + 1).min(w.saturating_sub(1)) {
            pix[ty][tx] = t_col;
        }
    }

    // Canopy: filled oval that sways with wind (lean shifts cx)
    let cr  = tree.canopy_r;
    let cx  = x as f64 + lean;
    // Canopy base sits at the top of the trunk
    let cy  = ground.saturating_sub(th) as f64;

    for dy in -(cr as i32)..=(cr as i32) {
        for dx in -((cr * 1.55) as i32)..=((cr * 1.55) as i32) {
            let ex = dx as f64 / (cr * 1.55);
            let ey = dy as f64 / cr;
            let d  = ex * ex + ey * ey;
            if d <= 1.0 {
                let px = (cx + dx as f64) as i32;
                let py = (cy + dy as f64) as i32;
                if px >= 0 && px < w as i32 && py >= 0 && py < ph as i32 {
                    // Darker at the edges for a subtle 3-D look
                    pix[py as usize][px as usize] = lerp(l_col, l_dark, d * 0.55);
                }
            }
        }
    }
}

/// Safe pixel set: bounds-checks before writing.
fn sp(pix: &mut Pix, w: usize, max_y: usize, x: i32, y: i32, col: Rgb) {
    if x >= 0 && x < w as i32 && y >= 0 && y < max_y as i32 {
        pix[y as usize][x as usize] = col;
    }
}

// ── Half-block composer ────────────────────────────────────────────────────────
//
// For each terminal row y:
//   upper pixel = pix[y*2][x]   → foreground color
//   lower pixel = pix[y*2+1][x] → background color
//   character   = ▀  (U+2580 UPPER HALF BLOCK)
//
// This encodes 2 rows of colour data into 1 terminal row at no extra cost.

fn half_blocks(pix: &Pix, w: usize, h: usize, color_prov: &ColorProvider, t_abs: f64) -> String {
    // We allocate less memory now because we skip redundant color codes!
    let mut out = String::with_capacity(w * h * 20);
    
    let mut last_fg: Option<Rgb> = None;
    let mut last_bg: Option<Rgb> = None;

    for y in 0..h {
        let upper = y * 2;
        let lower = y * 2 + 1;

        for x in 0..w {
            let base_fg = pix[upper][x];
            let base_bg = if lower < pix.len() { pix[lower][x] } else { (0, 0, 0) };

            // APPLY THE CAMERA FILTER HERE
            let fg = color_prov.tint(base_fg, t_abs, x as i32, upper as i32);
            let bg = color_prov.tint(base_bg, t_abs, x as i32, lower as i32);

            if Some(fg) != last_fg {
                out.push_str(&crate::ansi::rgb(fg.0, fg.1, fg.2));
                last_fg = Some(fg);
            }
            if Some(bg) != last_bg {
                out.push_str(&crate::ansi::bg_rgb(bg.0, bg.1, bg.2));
                last_bg = Some(bg);
            }
            out.push('▀');
        }
        
        // Reset terminal state at the end of every line
        out.push_str(crate::ansi::RESET);
        last_fg = None;
        last_bg = None;
        
        if y < h - 1 { out.push('\n'); }
    }

    out
}