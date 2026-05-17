// ===== src/modes/biome_colony.rs =====
//
// BiomeColonyMode
//
// A living alien terrarium / ecosystem scene.
// The idea is not "forest wallpaper" but a small simulated biome:
//
//   sky -> drifting spores -> procedural terrain -> glowing root network ->
//   crystal caves -> water pools -> wandering creatures -> fireflies -> rain cycles
//
// This is intentionally weird/organic, like a little terminal ant-farm planet.

use crate::ansi::{bg_rgb, rgb, RESET};
use crate::color::ColorProvider;
use crate::mode_base::Mode;
use rand::RngExt;
use std::f64::consts::TAU;

type Rgb = (u8, u8, u8);
type Pix = Vec<Vec<Rgb>>;

#[derive(Clone, Copy)]
struct Firefly {
    seed: f64,
    x: f64,
    y: f64,
    speed: f64,
    phase: f64,
}

#[derive(Clone, Copy)]
struct Spore {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    phase: f64,
}

#[derive(Clone, Copy)]
struct Creature {
    x: f64,
    direction: f64,
    speed: f64,
    seed: f64,
    mood_phase: f64,
}

#[derive(Clone, Copy)]
struct RainDrop {
    x: f64,
    y: f64,
    speed: f64,
    drift: f64,
}

pub struct BiomeColonyMode {
    speed: f64,
    color_provider: ColorProvider,
    time: f64,
    terrain_seed: f64,
    fireflies: Vec<Firefly>,
    spores: Vec<Spore>,
    creatures: Vec<Creature>,
    rain: Vec<RainDrop>,
}

impl BiomeColonyMode {
    pub fn new(speed: f64, color_provider: ColorProvider) -> Self {
        let mut rng = rand::rng();

        let fireflies = (0..48)
            .map(|_| Firefly {
                seed: rng.random_range(0.0..1000.0),
                x: rng.random_range(0.0..180.0),
                y: rng.random_range(5.0..55.0),
                speed: rng.random_range(0.15..0.55),
                phase: rng.random_range(0.0..TAU),
            })
            .collect();

        let spores = (0..120)
            .map(|_| Spore {
                x: rng.random_range(0.0..220.0),
                y: rng.random_range(0.0..80.0),
                vx: rng.random_range(-0.8..1.4),
                vy: rng.random_range(-0.18..0.45),
                phase: rng.random_range(0.0..TAU),
            })
            .collect();

        let creatures = (0..10)
            .map(|i| Creature {
                x: rng.random_range(0.0..220.0),
                direction: if i % 2 == 0 { 1.0 } else { -1.0 },
                speed: rng.random_range(1.8..5.4),
                seed: rng.random_range(0.0..1000.0),
                mood_phase: rng.random_range(0.0..TAU),
            })
            .collect();

        let rain = (0..170)
            .map(|_| RainDrop {
                x: rng.random_range(0.0..240.0),
                y: rng.random_range(-80.0..80.0),
                speed: rng.random_range(22.0..48.0),
                drift: rng.random_range(-3.5..1.5),
            })
            .collect();

        Self {
            speed,
            color_provider,
            time: 0.0,
            terrain_seed: rng.random_range(0.0..1000.0),
            fireflies,
            spores,
            creatures,
            rain,
        }
    }
}

impl Mode for BiomeColonyMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        let dt = dt * self.speed;
        let w = width.max(1) as f64;
        let ph = height.max(1) as f64 * 2.0;

        self.time += dt;

        for fly in &mut self.fireflies {
            fly.x += fly.speed * dt * 8.0;
            fly.y += (self.time * 1.5 + fly.phase).sin() * dt * 2.0;

            if fly.x > w + 5.0 {
                fly.x = -5.0;
            }
        }

        for spore in &mut self.spores {
            spore.x += spore.vx * dt + (self.time * 0.7 + spore.phase).sin() * dt * 0.7;
            spore.y += spore.vy * dt + (self.time * 0.45 + spore.phase).cos() * dt * 0.25;

            if spore.x < -2.0 { spore.x = w + 2.0; }
            if spore.x > w + 2.0 { spore.x = -2.0; }
            if spore.y < -2.0 { spore.y = ph * 0.55; }
            if spore.y > ph * 0.58 { spore.y = -2.0; }
        }

        for creature in &mut self.creatures {
            creature.x += creature.direction * creature.speed * dt;

            if creature.x < -10.0 {
                creature.x = w + 10.0;
            } else if creature.x > w + 10.0 {
                creature.x = -10.0;
            }
        }

        let storm = rain_strength(self.time);
        if storm > 0.05 {
            for drop in &mut self.rain {
                drop.x += drop.drift * dt;
                drop.y += drop.speed * dt * (0.5 + storm);

                if drop.y > ph + 5.0 {
                    drop.y = -10.0;
                    drop.x = (drop.x + 37.0).rem_euclid(w.max(1.0));
                }

                if drop.x < 0.0 {
                    drop.x += w;
                } else if drop.x >= w {
                    drop.x -= w;
                }
            }
        }
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w = width.max(1) as usize;
        let h = height.max(1) as usize;
        let ph = h * 2;

        let mut pix: Pix = vec![vec![(0, 0, 0); w]; ph];

        paint_sky(&mut pix, w, ph, self.time);
        paint_distant_fungus(&mut pix, w, ph, self.time);

        for x in 0..w {
            let ground = terrain_height(x, w, ph, self.terrain_seed, self.time);
            paint_terrain_column(&mut pix, x, ground, ph, self.time);
            paint_roots_column(&mut pix, x, ground, ph, self.terrain_seed, self.time);
            paint_water_column(&mut pix, x, ground, ph, self.time);
            paint_crystals_column(&mut pix, x, ground, ph, self.terrain_seed, self.time);
        }

        for spore in &self.spores {
            paint_spore(&mut pix, w, ph, spore, self.time);
        }

        for creature in &self.creatures {
            paint_creature(&mut pix, w, ph, creature, self.terrain_seed, self.time);
        }

        for fly in &self.fireflies {
            paint_firefly(&mut pix, w, ph, fly, self.time);
        }

        paint_rain(&mut pix, w, ph, &self.rain, self.time);

        half_blocks(&pix, w, h, &self.color_provider, t_abs)
    }
}

fn lerp(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let c = |av: u8, bv: u8| (av as f64 + (bv as f64 - av as f64) * t) as u8;
    (c(a.0, b.0), c(a.1, b.1), c(a.2, b.2))
}

fn blend(bg: Rgb, fg: Rgb, alpha: f64) -> Rgb {
    lerp(bg, fg, alpha.clamp(0.0, 1.0))
}

fn brighten(c: Rgb, n: i32) -> Rgb {
    let b = |v: u8| (v as i32 + n).clamp(0, 255) as u8;
    (b(c.0), b(c.1), b(c.2))
}

fn darken(c: Rgb, f: f64) -> Rgb {
    let s = (1.0 - f).clamp(0.0, 1.0);
    (
        (c.0 as f64 * s) as u8,
        (c.1 as f64 * s) as u8,
        (c.2 as f64 * s) as u8,
    )
}

fn hash01(n: f64) -> f64 {
    (n.sin() * 43758.5453123).fract().abs()
}

fn fbm(x: f64, seed: f64) -> f64 {
    0.38 * (x * 0.035 + seed).sin()
        + 0.27 * (x * 0.081 + seed * 1.7).sin()
        + 0.18 * (x * 0.145 + seed * 0.9).sin()
        + 0.10 * (x * 0.31 + seed * 2.3).sin()
        + 0.07 * (x * 0.65 + seed * 0.5).sin()
}

fn rain_strength(time: f64) -> f64 {
    let cycle = ((time * 0.055).sin() + 1.0) * 0.5;
    if cycle > 0.58 {
        ((cycle - 0.58) / 0.42).powf(1.4)
    } else {
        0.0
    }
}

fn terrain_height(x: usize, w: usize, ph: usize, seed: f64, time: f64) -> usize {
    let xf = x as f64 / w.max(1) as f64;
    let base = ph as f64 * 0.58;
    let hill = fbm(x as f64 + time * 1.2, seed);
    let big = (xf * TAU * 1.4 + seed).sin() * 0.08;
    (base + hill * ph as f64 * 0.10 + big * ph as f64)
        .clamp(ph as f64 * 0.38, ph as f64 * 0.76) as usize
}

fn paint_sky(pix: &mut Pix, w: usize, ph: usize, time: f64) {
    let storm = rain_strength(time);
    let top = lerp((7, 15, 28), (10, 12, 20), storm);
    let bottom = lerp((32, 58, 48), (28, 36, 44), storm);

    for y in 0..ph {
        let yf = y as f64 / ph.max(1) as f64;
        let mut col = lerp(top, bottom, yf.powf(0.75));

        let pulse = ((time * 0.18 + yf * 6.0).sin() + 1.0) * 0.5;
        col = blend(col, (28, 80, 62), pulse * 0.05);

        for x in 0..w {
            pix[y][x] = col;
        }
    }
}

fn paint_distant_fungus(pix: &mut Pix, w: usize, ph: usize, time: f64) {
    let base_y = (ph as f64 * 0.54) as i32;

    for i in 0..18 {
        let seed = i as f64 * 19.37;
        let x = (hash01(seed) * w as f64) as i32;
        let h = (8.0 + hash01(seed + 2.0) * ph as f64 * 0.16) as i32;
        let cap_r = (3.0 + hash01(seed + 4.0) * 8.0) as i32;
        let sway = ((time * 0.35 + seed).sin() * 2.0) as i32;

        let stem_col = (26, 65, 48);
        let cap_col = (60, 95, 80);

        for y in base_y - h..base_y {
            let px = x + sway * (base_y - y) / h.max(1);
            set_blend(pix, w, ph, px, y, stem_col, 0.65);
        }

        let cap_y = base_y - h;
        for dy in -cap_r / 2..=cap_r / 2 {
            for dx in -cap_r..=cap_r {
                let d = dx as f64 * dx as f64 / (cap_r.max(1) * cap_r.max(1)) as f64
                    + dy as f64 * dy as f64 / ((cap_r / 2).max(1) * (cap_r / 2).max(1)) as f64;

                if d <= 1.0 {
                    set_blend(pix, w, ph, x + dx + sway, cap_y + dy, cap_col, 0.55);
                }
            }
        }
    }
}

fn paint_terrain_column(pix: &mut Pix, x: usize, ground: usize, ph: usize, time: f64) {
    let grass = (42, 105, 55);
    let soil = (46, 42, 32);
    let deep = (18, 20, 24);

    for y in ground..ph {
        let depth = (y - ground) as f64 / (ph - ground).max(1) as f64;
        let mut col = if depth < 0.12 {
            lerp(grass, soil, depth / 0.12)
        } else {
            lerp(soil, deep, ((depth - 0.12) / 0.88).powf(0.75))
        };

        if depth < 0.05 {
            let ripple = ((x as f64 * 0.3 + time * 2.0).sin() + 1.0) * 0.5;
            col = brighten(col, (ripple * 22.0) as i32);
        }

        pix[y][x] = col;
    }
}

fn paint_roots_column(pix: &mut Pix, x: usize, ground: usize, ph: usize, seed: f64, time: f64) {
    for y in ground + 3..ph {
        let depth = (y - ground) as f64 / (ph - ground).max(1) as f64;
        let wave = (x as f64 * 0.21 + y as f64 * 0.27 + seed + time * 0.18).sin().abs();
        let branch = hash01((x / 2) as f64 * 8.1 + (y / 3) as f64 * 5.7 + seed);

        if wave < 0.055 && branch > 0.32 {
            let glow = (1.0 - depth).clamp(0.0, 1.0);
            pix[y][x] = blend(pix[y][x], (95, 255, 135), 0.18 + glow * 0.35);
        }
    }
}

fn paint_water_column(pix: &mut Pix, x: usize, ground: usize, ph: usize, time: f64) {
    let pool_level = (ph as f64 * 0.80) as usize;

    if ground > pool_level {
        for y in pool_level..ground {
            let shimmer = ((x as f64 * 0.42 + y as f64 * 0.19 + time * 4.0).sin() + 1.0) * 0.5;
            let col = lerp((20, 75, 92), (85, 210, 195), shimmer);
            pix[y][x] = blend(pix[y][x], col, 0.72);
        }
    }
}

fn paint_crystals_column(pix: &mut Pix, x: usize, ground: usize, ph: usize, seed: f64, time: f64) {
    for y in ground + 10..ph.saturating_sub(2) {
        let h = hash01((x / 3) as f64 * 12.1 + (y / 5) as f64 * 8.3 + seed);
        if h > 0.985 {
            let pulse = ((time * 2.5 + h * TAU).sin() + 1.0) * 0.5;
            let col = lerp((80, 90, 180), (190, 120, 255), pulse);

            for dy in -2..=2 {
                set_blend(pix, x + 1, ph, x as i32, y as i32 + dy, col, 0.80 - dy.abs() as f64 * 0.12);
            }
        }
    }
}

fn paint_spore(pix: &mut Pix, w: usize, ph: usize, spore: &Spore, time: f64) {
    let x = spore.x.round() as i32;
    let y = spore.y.round() as i32;

    let pulse = ((time * 1.6 + spore.phase).sin() + 1.0) * 0.5;
    let col = lerp((60, 130, 80), (165, 255, 150), pulse);

    set_blend(pix, w, ph, x, y, col, 0.16 + pulse * 0.22);
}

fn paint_firefly(pix: &mut Pix, w: usize, ph: usize, fly: &Firefly, time: f64) {
    let x = fly.x.round() as i32;
    let y = (fly.y + (time * 1.7 + fly.phase).sin() * 3.0).round() as i32;

    let pulse = ((time * 5.0 + fly.phase + fly.seed).sin() + 1.0) * 0.5;
    let col = lerp((120, 160, 70), (240, 255, 95), pulse);

    paint_glow(pix, w, ph, x, y, 3, col, 0.12 + pulse * 0.45);
}

fn paint_creature(pix: &mut Pix, w: usize, ph: usize, creature: &Creature, seed: f64, time: f64) {
    let x = creature.x.round() as i32;
    if x < -5 || x > w as i32 + 5 {
        return;
    }

    let ground = terrain_height(x.clamp(0, w as i32 - 1) as usize, w, ph, seed, time) as i32;
    let y = ground - 2;

    let mood = ((time * 1.8 + creature.mood_phase).sin() + 1.0) * 0.5;
    let body = lerp((110, 170, 120), (185, 230, 150), mood);

    paint_glow(pix, w, ph, x, y, 3, body, 0.12);

    for dy in -1..=1 {
        for dx in -2..=2 {
            let oval = dx * dx + dy * dy * 3 <= 5;
            if oval {
                set_blend(pix, w, ph, x + dx, y + dy, body, 0.95);
            }
        }
    }

    let eye_x = x + creature.direction.signum() as i32 * 2;
    set_blend(pix, w, ph, eye_x, y - 1, (255, 245, 160), 0.95);

    // Little walking legs.
    let step = ((time * 8.0 + creature.seed).sin() > 0.0) as i32;
    set_blend(pix, w, ph, x - 1, y + 2 + step, darken(body, 0.35), 0.9);
    set_blend(pix, w, ph, x + 1, y + 2 + (1 - step), darken(body, 0.35), 0.9);
}

fn paint_rain(pix: &mut Pix, w: usize, ph: usize, rain: &[RainDrop], time: f64) {
    let strength = rain_strength(time);
    if strength <= 0.05 {
        return;
    }

    for drop in rain {
        let x = drop.x.round() as i32;
        let y = drop.y.round() as i32;

        for i in 0..3 {
            set_blend(
                pix,
                w,
                ph,
                x - i,
                y + i,
                (100, 170, 190),
                strength * (0.45 - i as f64 * 0.10),
            );
        }
    }
}

fn paint_glow(pix: &mut Pix, w: usize, ph: usize, cx: i32, cy: i32, r: i32, col: Rgb, power: f64) {
    for dy in -r..=r {
        for dx in -r..=r {
            let d = ((dx * dx + dy * dy) as f64).sqrt();
            if d <= r as f64 {
                let px = cx + dx;
                let py = cy + dy;
                if px >= 0 && px < w as i32 && py >= 0 && py < ph as i32 {
                    let a = (1.0 - d / r.max(1) as f64).powf(1.4) * power;
                    pix[py as usize][px as usize] = blend(pix[py as usize][px as usize], col, a);
                }
            }
        }
    }
}

fn set_blend(pix: &mut Pix, w: usize, ph: usize, x: i32, y: i32, col: Rgb, alpha: f64) {
    if x >= 0 && x < w as i32 && y >= 0 && y < ph as i32 {
        pix[y as usize][x as usize] = blend(pix[y as usize][x as usize], col, alpha);
    }
}

fn half_blocks(pix: &Pix, w: usize, h: usize, color_provider: &ColorProvider, t_abs: f64) -> String {
    let mut out = String::with_capacity(w * h * 24);

    let mut last_fg: Option<Rgb> = None;
    let mut last_bg: Option<Rgb> = None;

    for y in 0..h {
        let upper = y * 2;
        let lower = y * 2 + 1;

        for x in 0..w {
            let base_fg = pix[upper][x];
            let base_bg = if lower < pix.len() { pix[lower][x] } else { (0, 0, 0) };

            let fg = color_provider.tint(base_fg, t_abs, x as i32, upper as i32);
            let bg = color_provider.tint(base_bg, t_abs, x as i32, lower as i32);

            if Some(fg) != last_fg {
                out.push_str(&rgb(fg.0, fg.1, fg.2));
                last_fg = Some(fg);
            }

            if Some(bg) != last_bg {
                out.push_str(&bg_rgb(bg.0, bg.1, bg.2));
                last_bg = Some(bg);
            }

            out.push('▀');
        }

        out.push_str(RESET);
        last_fg = None;
        last_bg = None;

        if y < h - 1 {
            out.push('\n');
        }
    }

    out
}
