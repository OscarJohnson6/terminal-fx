// ===== src/modes/aurora_city.rs =====
//
// AuroraCityMode
//
// A neon skyline scene with aurora bands, procedural buildings, animated windows,
// drifting sky traffic, star twinkles, scanline glow, and occasional meteor streaks.
//
// This is intentionally more "wallpaper scene" than "toy effect":
//   sky gradient -> aurora curtains -> stars -> meteors -> traffic lanes ->
//   skyline silhouettes -> window lights -> rooftop beacons
//
// Rendering style:
//   Uses half-block pixel rendering like LandscapeMode/SkyHarborMode for smoother visuals.

use crate::ansi::{bg_rgb, rgb, RESET};
use crate::color::ColorProvider;
use crate::mode_base::Mode;
use rand::RngExt;
use std::f64::consts::TAU;

type Rgb = (u8, u8, u8);
type Pix = Vec<Vec<Rgb>>;

#[derive(Clone, Copy)]
struct Building {
    x: i32,
    w: i32,
    h: i32,
    roof: RoofKind,
    seed: f64,
    tint: Rgb,
}

#[derive(Clone, Copy)]
enum RoofKind {
    Flat,
    Antenna,
    Spire,
    Dome,
}

#[derive(Clone, Copy)]
struct SkyCar {
    x: f64,
    y_frac: f64,
    speed: f64,
    lane_phase: f64,
    direction: f64,
    color: Rgb,
}

#[derive(Clone, Copy)]
struct Meteor {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    life: f64,
    max_life: f64,
    cooldown: f64,
}

pub struct AuroraCityMode {
    speed: f64,
    color_provider: ColorProvider,
    time: f64,
    buildings: Vec<Building>,
    skycars: Vec<SkyCar>,
    meteors: Vec<Meteor>,
    last_w: u16,
    last_h: u16,
}

impl AuroraCityMode {
    pub fn new(speed: f64, color_provider: ColorProvider) -> Self {
        let mut rng = rand::rng();

        let skycars = (0..16)
            .map(|i| {
                let direction = if i % 2 == 0 { 1.0 } else { -1.0 };
                SkyCar {
                    x: rng.random_range(-140.0..260.0),
                    y_frac: rng.random_range(0.16..0.62),
                    speed: rng.random_range(10.0..34.0),
                    lane_phase: rng.random_range(0.0..TAU),
                    direction,
                    color: if i % 3 == 0 {
                        (255, 105, 190)
                    } else if i % 3 == 1 {
                        (90, 220, 255)
                    } else {
                        (255, 210, 95)
                    },
                }
            })
            .collect();

        let meteors = (0..4)
            .map(|_| Meteor {
                x: 0.0,
                y: 0.0,
                vx: 0.0,
                vy: 0.0,
                life: 0.0,
                max_life: 0.0,
                cooldown: rng.random_range(3.0..12.0),
            })
            .collect();

        Self {
            speed,
            color_provider,
            time: 0.0,
            buildings: Vec::new(),
            skycars,
            meteors,
            last_w: 0,
            last_h: 0,
        }
    }

    fn rebuild_city(&mut self, width: u16, height: u16) {
        self.buildings.clear();

        let mut rng = rand::rng();
        let w = width as i32;
        let ph = height as i32 * 2;

        let mut x = 0;
        while x < w {
            let bw = rng.random_range(4..11);
            let bh = rng.random_range((ph as f64 * 0.18) as i32..(ph as f64 * 0.52) as i32).max(4);
            let roof = match rng.random_range(0..4) {
                0 => RoofKind::Flat,
                1 => RoofKind::Antenna,
                2 => RoofKind::Spire,
                _ => RoofKind::Dome,
            };

            let tint = match rng.random_range(0..4) {
                0 => (18, 22, 45),
                1 => (24, 20, 48),
                2 => (17, 28, 42),
                _ => (28, 22, 36),
            };

            self.buildings.push(Building {
                x,
                w: bw,
                h: bh,
                roof,
                seed: rng.random_range(0.0..1000.0),
                tint,
            });

            x += bw + rng.random_range(0..3);
        }

        self.last_w = width;
        self.last_h = height;
    }
}

impl Mode for AuroraCityMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        if width != self.last_w || height != self.last_h || self.buildings.is_empty() {
            self.rebuild_city(width, height);
        }

        let dt = dt * self.speed;
        self.time += dt;

        let w = width.max(1) as f64;
        let ph = height.max(1) as f64 * 2.0;

        for car in &mut self.skycars {
            car.x += car.direction * car.speed * dt;
            if car.direction > 0.0 && car.x > w + 40.0 {
                car.x = -40.0;
            } else if car.direction < 0.0 && car.x < -40.0 {
                car.x = w + 40.0;
            }
        }

        let mut rng = rand::rng();
        for meteor in &mut self.meteors {
            if meteor.life > 0.0 {
                meteor.life -= dt;
                meteor.x += meteor.vx * dt;
                meteor.y += meteor.vy * dt;
            } else {
                meteor.cooldown -= dt;
                if meteor.cooldown <= 0.0 {
                    meteor.x = rng.random_range(0.0..w);
                    meteor.y = rng.random_range(2.0..ph * 0.26);
                    meteor.vx = rng.random_range(-42.0..-18.0);
                    meteor.vy = rng.random_range(10.0..24.0);
                    meteor.max_life = rng.random_range(0.65..1.25);
                    meteor.life = meteor.max_life;
                    meteor.cooldown = rng.random_range(7.0..20.0);
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
        paint_stars(&mut pix, w, ph, self.time);
        paint_aurora(&mut pix, w, ph, self.time);

        for meteor in &self.meteors {
            paint_meteor(&mut pix, w, ph, meteor);
        }

        for car in &self.skycars {
            paint_skycar(&mut pix, w, ph, car, self.time);
        }

        paint_city_glow(&mut pix, w, ph, self.time);

        for building in &self.buildings {
            paint_building(&mut pix, w, ph, building, self.time);
        }

        paint_scanlines(&mut pix, w, ph, self.time);

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

fn paint_sky(pix: &mut Pix, w: usize, ph: usize, time: f64) {
    let top = (3, 5, 25);
    let mid = (18, 14, 48);
    let horizon = (54, 28, 68);

    for y in 0..ph {
        let yf = y as f64 / ph.max(1) as f64;
        let mut col = if yf < 0.55 {
            lerp(top, mid, yf / 0.55)
        } else {
            lerp(mid, horizon, (yf - 0.55) / 0.45)
        };

        let pulse = ((time * 0.18 + yf * 8.0).sin() + 1.0) * 0.5;
        col = blend(col, (35, 10, 70), pulse * 0.045);

        for x in 0..w {
            pix[y][x] = col;
        }
    }
}

fn paint_stars(pix: &mut Pix, w: usize, ph: usize, time: f64) {
    let max_y = (ph as f64 * 0.48) as usize;

    for i in 0..280usize {
        let sx = (i.wrapping_mul(2654435761) >> 4) % w.max(1);
        let sy = (i.wrapping_mul(2246822519) >> 4) % max_y.max(1);
        let twinkle = ((time * 2.2 + i as f64 * 0.73).sin() + 1.0) * 0.5;
        let b = (35.0 + twinkle * 190.0) as u8;

        if b > 85 {
            pix[sy][sx] = blend(pix[sy][sx], (b, b, 230), 0.65);
        }
    }
}

fn paint_aurora(pix: &mut Pix, w: usize, ph: usize, time: f64) {
    let max_y = (ph as f64 * 0.56) as usize;

    for y in 0..max_y {
        let yf = y as f64 / ph.max(1) as f64;

        for x in 0..w {
            let xf = x as f64 / w.max(1) as f64;

            let curtain_a = ((xf * 10.0 + time * 0.55).sin() + 1.0) * 0.5;
            let curtain_b = ((xf * 17.0 - time * 0.38 + yf * 4.0).sin() + 1.0) * 0.5;
            let center = 0.16 + curtain_a * 0.16 + curtain_b * 0.05;

            let dist = (yf - center).abs();
            let band = (1.0 - dist / 0.115).clamp(0.0, 1.0);
            let vertical_fade = (1.0 - yf / 0.58).clamp(0.0, 1.0);
            let strength = band.powf(1.7) * vertical_fade;

            if strength > 0.05 {
                let color_shift = ((xf * 7.0 + time * 0.45).sin() + 1.0) * 0.5;
                let aurora_col = lerp((30, 230, 170), (175, 90, 255), color_shift);
                pix[y][x] = blend(pix[y][x], aurora_col, strength * 0.58);
            }
        }
    }
}

fn paint_meteor(pix: &mut Pix, w: usize, ph: usize, meteor: &Meteor) {
    if meteor.life <= 0.0 {
        return;
    }

    let fade = (meteor.life / meteor.max_life).clamp(0.0, 1.0);
    let head = (235, 245, 255);

    for i in 0..18 {
        let t = i as f64 / 18.0;
        let x = meteor.x - meteor.vx * t * 0.045;
        let y = meteor.y - meteor.vy * t * 0.045;

        let px = x.round() as i32;
        let py = y.round() as i32;

        if px >= 0 && px < w as i32 && py >= 0 && py < ph as i32 {
            let alpha = fade * (1.0 - t).powf(1.2);
            let col = lerp((90, 140, 255), head, 1.0 - t);
            pix[py as usize][px as usize] = blend(pix[py as usize][px as usize], col, alpha);
        }
    }
}

fn paint_skycar(pix: &mut Pix, w: usize, ph: usize, car: &SkyCar, time: f64) {
    let x = car.x.round() as i32;
    let lane_wave = (time * 0.85 + car.lane_phase).sin() * 2.0;
    let y = (car.y_frac * ph as f64 + lane_wave).round() as i32;

    let dir = car.direction.signum() as i32;

    for i in 0..9 {
        let px = x - dir * i;
        if px >= 0 && px < w as i32 && y >= 0 && y < ph as i32 {
            let fade = 1.0 - i as f64 / 9.0;
            pix[y as usize][px as usize] = blend(pix[y as usize][px as usize], car.color, fade * 0.70);
        }
    }

    for dy in -1..=1 {
        for dx in -2..=2 {
            let px = x + dx;
            let py = y + dy;
            if px >= 0 && px < w as i32 && py >= 0 && py < ph as i32 {
                let body = if dy == 0 { (180, 200, 220) } else { darken(car.color, 0.35) };
                pix[py as usize][px as usize] = blend(pix[py as usize][px as usize], body, 0.72);
            }
        }
    }

    let nose_x = x + dir * 3;
    if nose_x >= 0 && nose_x < w as i32 && y >= 0 && y < ph as i32 {
        pix[y as usize][nose_x as usize] = blend(pix[y as usize][nose_x as usize], (245, 255, 255), 0.95);
    }
}

fn paint_city_glow(pix: &mut Pix, w: usize, ph: usize, time: f64) {
    let start_y = (ph as f64 * 0.50) as usize;

    for y in start_y..ph {
        let yf = (y - start_y) as f64 / (ph - start_y).max(1) as f64;
        let pulse = ((time * 0.7 + yf * 4.0).sin() + 1.0) * 0.5;
        let glow = lerp((55, 20, 90), (20, 130, 165), pulse);

        for x in 0..w {
            let skyline_alpha = yf.powf(1.3) * 0.22;
            pix[y][x] = blend(pix[y][x], glow, skyline_alpha);
        }
    }
}

fn paint_building(pix: &mut Pix, w: usize, ph: usize, building: &Building, time: f64) {
    let ground = ph as i32 - 1;
    let top = ground - building.h;

    for y in top..=ground {
        for x in building.x..building.x + building.w {
            if x < 0 || x >= w as i32 || y < 0 || y >= ph as i32 {
                continue;
            }

            let edge = x == building.x || x == building.x + building.w - 1;
            let mut col = building.tint;
            if edge {
                col = darken(col, 0.22);
            }

            let vertical = (y - top) as f64 / building.h.max(1) as f64;
            col = blend(col, (8, 10, 22), vertical * 0.28);

            pix[y as usize][x as usize] = col;
        }
    }

    paint_windows(pix, w, ph, building, top, ground, time);
    paint_roof(pix, w, ph, building, top, time);
}

fn paint_windows(pix: &mut Pix, w: usize, ph: usize, building: &Building, top: i32, ground: i32, time: f64) {
    for y in top + 3..ground - 1 {
        if y % 4 != 0 {
            continue;
        }

        for x in building.x + 1..building.x + building.w - 1 {
            if x % 3 != 0 {
                continue;
            }

            if x < 0 || x >= w as i32 || y < 0 || y >= ph as i32 {
                continue;
            }

            let h = hash01(building.seed + x as f64 * 8.91 + y as f64 * 4.31);
            let flicker = ((time * 0.8 + h * TAU).sin() + 1.0) * 0.5;
            let lit = h > 0.28 && flicker > 0.18;

            if lit {
                let color_pick = hash01(building.seed + x as f64 * 2.0);
                let col = if color_pick < 0.33 {
                    (255, 210, 115)
                } else if color_pick < 0.66 {
                    (110, 230, 255)
                } else {
                    (255, 105, 190)
                };

                pix[y as usize][x as usize] = blend(pix[y as usize][x as usize], col, 0.92);

                if y + 1 < ph as i32 {
                    pix[(y + 1) as usize][x as usize] =
                        blend(pix[(y + 1) as usize][x as usize], col, 0.22);
                }
            }
        }
    }
}

fn paint_roof(pix: &mut Pix, w: usize, ph: usize, building: &Building, top: i32, time: f64) {
    let cx = building.x + building.w / 2;

    match building.roof {
        RoofKind::Flat => {
            for x in building.x..building.x + building.w {
                set_blend(pix, w, ph, x, top - 1, brighten(building.tint, 18), 0.9);
            }
        }
        RoofKind::Antenna => {
            for x in building.x..building.x + building.w {
                set_blend(pix, w, ph, x, top - 1, brighten(building.tint, 18), 0.9);
            }
            for y in top - 8..top {
                set_blend(pix, w, ph, cx, y, (85, 95, 120), 0.9);
            }

            let pulse = ((time * 5.0 + building.seed).sin() + 1.0) * 0.5;
            paint_glow(pix, w, ph, cx, top - 9, 4, (255, 75, 120), 0.15 + pulse * 0.45);
        }
        RoofKind::Spire => {
            for i in 0..7 {
                let y = top - i;
                for dx in -i / 2..=i / 2 {
                    set_blend(pix, w, ph, cx + dx, y, brighten(building.tint, 26), 0.9);
                }
            }
            paint_glow(pix, w, ph, cx, top - 7, 3, (120, 220, 255), 0.35);
        }
        RoofKind::Dome => {
            let r = (building.w / 2).max(2);
            for dy in -r..=0 {
                for dx in -r..=r {
                    let d = (dx * dx + dy * dy) as f64 / (r * r).max(1) as f64;
                    if d <= 1.0 {
                        set_blend(pix, w, ph, cx + dx, top + dy, brighten(building.tint, 35), 0.88);
                    }
                }
            }
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
                    let a = (1.0 - d / r.max(1) as f64).powf(1.5) * power;
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

fn paint_scanlines(pix: &mut Pix, w: usize, ph: usize, time: f64) {
    let sweep = ((time * 18.0) as usize) % ph.max(1);

    for y in (0..ph).step_by(4) {
        for x in 0..w {
            pix[y][x] = darken(pix[y][x], 0.08);
        }
    }

    for offset in 0..3 {
        let y = (sweep + offset) % ph.max(1);
        let alpha = 0.08 * (1.0 - offset as f64 / 3.0);
        for x in 0..w {
            pix[y][x] = blend(pix[y][x], (120, 210, 255), alpha);
        }
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
