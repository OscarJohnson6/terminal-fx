// ===== src/modes/tidal_beach.rs =====
//
// TidalBeachMode
//
// Smooth half-block shoreline mode.
// Waves move in/out, foam crawls across the sand, small objects wash ashore,
// birds skim the surf, and the tide refreshes the beach over time.
//
// This is the smoother visual remake of BeachShoreMode.
// The old ASCII beach was charming, but water + sand + foam looks better with
// the half-block pixel renderer.

use crate::ansi::{bg_rgb, rgb, RESET};
use crate::color::ColorProvider;
use crate::mode_base::Mode;
use rand::RngExt;
use std::f64::consts::TAU;

type Rgb = (u8, u8, u8);
type Pix = Vec<Vec<Rgb>>;

#[derive(Clone, Copy)]
enum ShoreThing {
    Shell,
    Bottle,
    Starfish,
    Driftwood,
    Kelp,
    Coin,
}

impl ShoreThing {
    fn color(self) -> Rgb {
        match self {
            ShoreThing::Shell => (246, 218, 178),
            ShoreThing::Bottle => (80, 205, 180),
            ShoreThing::Starfish => (255, 125, 90),
            ShoreThing::Driftwood => (132, 88, 52),
            ShoreThing::Kelp => (42, 145, 76),
            ShoreThing::Coin => (245, 205, 88),
        }
    }
}

#[derive(Clone, Copy)]
struct WashedItem {
    x: f64,
    y: f64,
    kind: ShoreThing,
    age: f64,
    phase: f64,
}

#[derive(Clone, Copy)]
struct Bird {
    x: f64,
    y: f64,
    vx: f64,
    phase: f64,
}

#[derive(Clone, Copy)]
struct Crab {
    x: f64,
    y: f64,
    vx: f64,
    phase: f64,
}

#[derive(Clone, Copy)]
struct Foam {
    x: f64,
    y: f64,
    life: f64,
    max_life: f64,
    phase: f64,
}

#[derive(Clone, Copy)]
struct Cloud {
    x: f64,
    y_frac: f64,
    rx: f64,
    ry: f64,
    speed: f64,
}

pub struct TidalBeachMode {
    speed: f64,
    color_provider: ColorProvider,
    time: f64,
    items: Vec<WashedItem>,
    birds: Vec<Bird>,
    crabs: Vec<Crab>,
    foam: Vec<Foam>,
    clouds: Vec<Cloud>,
    spawn_timer: f64,
    last_dims: Option<(u16, u16)>,
}

impl TidalBeachMode {
    pub fn new(speed: f64, color_provider: ColorProvider) -> Self {
        let mut rng = rand::rng();

        let birds = (0..8)
            .map(|i| Bird {
                x: rng.random_range(-40.0..180.0),
                y: rng.random_range(6.0..28.0),
                vx: if i % 2 == 0 {
                    rng.random_range(8.0..22.0)
                } else {
                    -rng.random_range(8.0..22.0)
                },
                phase: rng.random_range(0.0..TAU),
            })
            .collect();

        let crabs = (0..5)
            .map(|i| Crab {
                x: rng.random_range(0.0..180.0),
                y: 70.0,
                vx: if i % 2 == 0 { 4.0 } else { -4.0 },
                phase: rng.random_range(0.0..TAU),
            })
            .collect();

        let clouds = (0..7)
            .map(|_| Cloud {
                x: rng.random_range(-100.0..220.0),
                y_frac: rng.random_range(0.03..0.30),
                rx: rng.random_range(12.0..40.0),
                ry: rng.random_range(3.0..9.0),
                speed: rng.random_range(0.9..3.4),
            })
            .collect();

        Self {
            speed,
            color_provider,
            time: 0.0,
            items: Vec::new(),
            birds,
            crabs,
            foam: Vec::new(),
            clouds,
            spawn_timer: 0.0,
            last_dims: None,
        }
    }

    fn reset_size(&mut self, width: u16, height: u16) {
        let mut rng = rand::rng();
        let w = width.max(1) as f64;
        let ph = height.max(1) as f64 * 2.0;
        let sand = sand_start(ph);

        for crab in &mut self.crabs {
            crab.x = rng.random_range(0.0..w);
            crab.y = rng.random_range(sand + 8.0..(ph - 3.0).max(sand + 10.0));
        }

        self.items.clear();
        self.foam.clear();
        self.last_dims = Some((width, height));
    }

    fn spawn_item(&mut self, width: u16, height: u16) {
        let mut rng = rand::rng();
        let w = width.max(1) as f64;
        let ph = height.max(1) as f64 * 2.0;
        let tide = tide_line(ph, self.time);
        let sand = sand_start(ph);

        let kind = match rng.random_range(0..6) {
            0 => ShoreThing::Shell,
            1 => ShoreThing::Bottle,
            2 => ShoreThing::Starfish,
            3 => ShoreThing::Driftwood,
            4 => ShoreThing::Kelp,
            _ => ShoreThing::Coin,
        };

        self.items.push(WashedItem {
            x: rng.random_range(4.0..(w - 4.0).max(5.0)),
            y: rng.random_range(tide + 2.0..(sand + 20.0).min(ph - 3.0).max(tide + 3.0)),
            kind,
            age: 0.0,
            phase: rng.random_range(0.0..TAU),
        });

        if self.items.len() > 34 {
            self.items.remove(0);
        }
    }
}

impl Mode for TidalBeachMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        if self.last_dims != Some((width, height)) {
            self.reset_size(width, height);
        }

        let dt = dt * self.speed;
        self.time += dt;

        let mut rng = rand::rng();
        let w = width.max(1) as f64;
        let ph = height.max(1) as f64 * 2.0;
        let tide = tide_line(ph, self.time);
        let sand = sand_start(ph);

        for c in &mut self.clouds {
            c.x += c.speed * dt;
            if c.x > w + c.rx * 2.0 {
                c.x = -c.rx * 2.0;
            }
        }

        for bird in &mut self.birds {
            bird.x += bird.vx * dt;
            bird.y += (self.time * 1.4 + bird.phase).sin() * dt * 2.0;

            if bird.vx > 0.0 && bird.x > w + 20.0 {
                bird.x = -20.0;
                bird.y = rng.random_range(6.0..ph * 0.28);
            } else if bird.vx < 0.0 && bird.x < -20.0 {
                bird.x = w + 20.0;
                bird.y = rng.random_range(6.0..ph * 0.28);
            }
        }

        for crab in &mut self.crabs {
            crab.phase += dt * 5.0;
            crab.x += crab.vx * dt;
            crab.y += (self.time * 0.8 + crab.phase).sin() * dt * 0.4;

            if crab.x < 2.0 {
                crab.x = 2.0;
                crab.vx = crab.vx.abs();
            } else if crab.x > w - 2.0 {
                crab.x = w - 2.0;
                crab.vx = -crab.vx.abs();
            }

            if rng.random_range(0.0..1.0) < 0.008 {
                crab.vx = -crab.vx;
            }
        }

        self.spawn_timer -= dt;
        if self.spawn_timer <= 0.0 {
            self.spawn_item(width, height);
            self.spawn_timer = rng.random_range(0.7..2.1);
        }

        // Foam appears along the moving tide edge.
        for _ in 0..5 {
            if rng.random_range(0.0..1.0) < 0.65 {
                let x = rng.random_range(0.0..w);
                self.foam.push(Foam {
                    x,
                    y: tide + rng.random_range(-1.2..1.2),
                    life: rng.random_range(0.7..1.9),
                    max_life: 1.9,
                    phase: rng.random_range(0.0..TAU),
                });
            }
        }

        for f in &mut self.foam {
            f.x += (self.time * 0.9 + f.phase).sin() * dt * 3.5;
            f.y += dt * 1.4;
            f.life -= dt;
        }

        self.foam.retain(|f| f.life > 0.0);

        for item in &mut self.items {
            item.age += dt;
            let wave_touch = item.y <= tide + 2.0;
            if wave_touch {
                item.y -= dt * rng.random_range(0.3..2.0);
                item.x += dt * rng.random_range(-2.0..2.0);
            } else {
                item.y += ((self.time * 0.9 + item.phase).sin()) * dt * 0.06;
            }
        }

        // Crabs remove nearby small items.
        let mut remove = Vec::new();
        for crab in &self.crabs {
            for (i, item) in self.items.iter().enumerate() {
                let dx = item.x - crab.x;
                let dy = item.y - crab.y;
                if (dx * dx + dy * dy).sqrt() < 4.0 {
                    if rng.random_range(0.0..1.0) < 0.025 {
                        remove.push(i);
                    }
                }
            }
        }

        remove.sort_unstable();
        remove.dedup();
        for i in remove.into_iter().rev() {
            if i < self.items.len() {
                self.items.remove(i);
            }
        }

        // Tide clears older/water-covered objects.
        self.items.retain(|item| item.age < 34.0 && item.y > tide - 6.0 && item.y < ph - 1.0);

        if self.foam.len() > 260 {
            let drop = self.foam.len() - 260;
            self.foam.drain(..drop);
        }

        let _ = sand;
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w = width.max(1) as usize;
        let h = height.max(1) as usize;
        let ph = h * 2;

        let mut pix: Pix = vec![vec![(0, 0, 0); w]; ph];

        paint_sky(&mut pix, w, ph, self.time);
        paint_clouds(&mut pix, w, ph, &self.clouds);
        paint_water_and_sand(&mut pix, w, ph, self.time);

        for f in &self.foam {
            paint_foam(&mut pix, w, ph, f);
        }

        for item in &self.items {
            paint_item(&mut pix, w, ph, item, self.time);
        }

        for crab in &self.crabs {
            paint_crab(&mut pix, w, ph, crab, self.time);
        }

        for bird in &self.birds {
            paint_bird(&mut pix, w, ph, bird, self.time);
        }

        half_blocks(&pix, w, h, &self.color_provider, t_abs)
    }
}

fn sand_start(ph: f64) -> f64 {
    ph * 0.61
}

fn tide_line(ph: f64, time: f64) -> f64 {
    sand_start(ph) - 4.5 + (time * 0.58).sin() * 6.0 + (time * 1.35).sin() * 1.4
}

fn lerp(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let c = |av: u8, bv: u8| (av as f64 + (bv as f64 - av as f64) * t) as u8;
    (c(a.0, b.0), c(a.1, b.1), c(a.2, b.2))
}

fn blend(bg: Rgb, fg: Rgb, alpha: f64) -> Rgb {
    let a = alpha.clamp(0.0, 1.0);
    let mix = |b: u8, f: u8| (b as f64 + (f as f64 - b as f64) * a) as u8;
    (mix(bg.0, fg.0), mix(bg.1, fg.1), mix(bg.2, fg.2))
}

fn darken(c: Rgb, f: f64) -> Rgb {
    let s = (1.0 - f).clamp(0.0, 1.0);
    (
        (c.0 as f64 * s) as u8,
        (c.1 as f64 * s) as u8,
        (c.2 as f64 * s) as u8,
    )
}

fn paint_sky(pix: &mut Pix, w: usize, ph: usize, time: f64) {
    let sky_h = (ph as f64 * 0.42) as usize;

    for y in 0..sky_h.min(ph) {
        let t = y as f64 / sky_h.max(1) as f64;
        let mut col = lerp((75, 155, 220), (190, 225, 245), t.powf(0.8));

        let warm = ((time * 0.05).sin() + 1.0) * 0.5;
        col = blend(col, (255, 190, 120), warm * 0.10);

        for x in 0..w {
            pix[y][x] = col;
        }
    }

    // Sun disk and glow.
    let sx = w as f64 * 0.78;
    let sy = ph as f64 * 0.13;
    paint_soft_circle(pix, w, ph, sx, sy, 11.0, (255, 230, 105), 0.95);
    paint_soft_circle(pix, w, ph, sx, sy, 24.0, (255, 210, 120), 0.10);
}

fn paint_clouds(pix: &mut Pix, w: usize, ph: usize, clouds: &[Cloud]) {
    for c in clouds {
        let cx = c.x as i32;
        let cy = (c.y_frac * ph as f64) as i32;

        for bump in -2..=2 {
            let bx = cx + bump * c.rx as i32 / 4;
            let rx = (c.rx * (0.45 + bump.abs() as f64 * 0.05)) as i32;
            let ry = c.ry as i32;

            for dy in -ry..=ry {
                for dx in -rx..=rx {
                    let ex = dx as f64 / rx.max(1) as f64;
                    let ey = dy as f64 / ry.max(1) as f64;
                    let d = ex * ex + ey * ey;

                    if d <= 1.0 {
                        let x = bx + dx;
                        let y = cy + dy;
                        if in_bounds(w, ph, x, y) {
                            let a = (1.0 - d).powf(0.55) * 0.62;
                            pix[y as usize][x as usize] =
                                blend(pix[y as usize][x as usize], (245, 248, 252), a);
                        }
                    }
                }
            }
        }
    }
}

fn paint_water_and_sand(pix: &mut Pix, w: usize, ph: usize, time: f64) {
    let sand = sand_start(ph as f64);
    let tide = tide_line(ph as f64, time);

    for y in 0..ph {
        let yf = y as f64;

        for x in 0..w {
            if yf < sand {
                let depth = (yf / sand.max(1.0)).clamp(0.0, 1.0);
                let wave = ((x as f64 * 0.28 + time * 3.0 + yf * 0.12).sin()
                    + (x as f64 * 0.08 - time * 1.4).cos())
                    * 0.5;

                let shallow = yf > tide - 3.0;
                let base = if shallow {
                    (90, 210, 220)
                } else {
                    (25, 118, 185)
                };

                let mut col = lerp((25, 145, 205), base, depth);
                col = blend(col, (230, 255, 250), wave.max(0.0) * 0.10);

                pix[y][x] = col;
            } else {
                let depth = ((yf - sand) / (ph as f64 - sand).max(1.0)).clamp(0.0, 1.0);
                let grain = ((x as f64 * 1.7 + yf * 0.8 + time * 0.15).sin() + 1.0) * 0.5;
                let mut col = lerp((225, 190, 125), (170, 130, 78), depth);
                col = blend(col, (245, 225, 165), grain * 0.08);

                // Wet sand near tide edge.
                let wet = (1.0 - ((yf - tide).abs() / 12.0)).clamp(0.0, 1.0);
                col = blend(col, (120, 155, 145), wet * 0.28);

                pix[y][x] = col;
            }
        }
    }

    // Foam line.
    for x in 0..w {
        let y = (tide + (x as f64 * 0.18 + time * 2.4).sin() * 1.7).round() as i32;
        for dy in -1..=1 {
            if in_bounds(w, ph, x as i32, y + dy) {
                let a = if dy == 0 { 0.80 } else { 0.32 };
                pix[(y + dy) as usize][x] = blend(pix[(y + dy) as usize][x], (240, 255, 250), a);
            }
        }
    }
}

fn paint_foam(pix: &mut Pix, w: usize, ph: usize, f: &Foam) {
    let fade = (f.life / f.max_life).clamp(0.0, 1.0);
    let x = f.x + f.phase.sin() * 1.5;
    paint_soft_circle(pix, w, ph, x, f.y, 2.4, (245, 255, 250), fade * 0.35);
}

fn paint_item(pix: &mut Pix, w: usize, ph: usize, item: &WashedItem, time: f64) {
    let bob = (time * 1.3 + item.phase).sin() * 0.4;
    let x = item.x;
    let y = item.y + bob;
    let col = item.kind.color();

    match item.kind {
        ShoreThing::Shell => {
            paint_soft_circle(pix, w, ph, x, y, 2.2, col, 0.82);
            paint_soft_circle(pix, w, ph, x - 0.8, y - 0.3, 0.8, (255, 240, 215), 0.65);
        }
        ShoreThing::Bottle => {
            paint_rect(pix, w, ph, x - 2.0, y - 0.8, 4.0, 1.8, col, 0.74);
            paint_rect(pix, w, ph, x + 1.5, y - 1.0, 2.0, 1.0, (150, 235, 220), 0.70);
        }
        ShoreThing::Starfish => {
            for i in 0..5 {
                let a = TAU * i as f64 / 5.0 + item.phase;
                paint_soft_circle(pix, w, ph, x + a.cos() * 1.8, y + a.sin() * 1.2, 1.2, col, 0.82);
            }
        }
        ShoreThing::Driftwood => {
            paint_rect(pix, w, ph, x - 3.0, y - 0.5, 6.0, 1.2, col, 0.86);
        }
        ShoreThing::Kelp => {
            for k in 0..5 {
                let yy = y + k as f64 * 0.8;
                let xx = x + (time * 1.2 + k as f64).sin() * 0.8;
                paint_soft_circle(pix, w, ph, xx, yy, 1.0, col, 0.65);
            }
        }
        ShoreThing::Coin => {
            paint_soft_circle(pix, w, ph, x, y, 1.5, col, 0.92);
            paint_soft_circle(pix, w, ph, x - 0.3, y - 0.3, 0.6, (255, 245, 180), 0.6);
        }
    }
}

fn paint_crab(pix: &mut Pix, w: usize, ph: usize, crab: &Crab, time: f64) {
    let x = crab.x;
    let y = crab.y + (time * 2.0 + crab.phase).sin() * 0.3;
    let col = (215, 80, 55);

    paint_soft_circle(pix, w, ph, x, y, 2.0, col, 0.88);
    paint_soft_circle(pix, w, ph, x - 2.3, y - 0.8, 0.9, col, 0.82);
    paint_soft_circle(pix, w, ph, x + 2.3, y - 0.8, 0.9, col, 0.82);

    let step = (time * 7.0 + crab.phase).sin();
    for side in [-1.0, 1.0] {
        for leg in 0..3 {
            let lx = x + side * (1.5 + leg as f64 * 0.8);
            let ly = y + 1.2 + step * side * 0.3;
            paint_soft_circle(pix, w, ph, lx, ly, 0.55, darken(col, 0.25), 0.75);
        }
    }
}

fn paint_bird(pix: &mut Pix, w: usize, ph: usize, bird: &Bird, time: f64) {
    let x = bird.x;
    let y = bird.y + (time * 1.7 + bird.phase).sin() * 1.3;
    let wing = (time * 8.0 + bird.phase).sin();
    let col = (35, 35, 42);

    paint_soft_circle(pix, w, ph, x, y, 1.1, col, 0.9);
    paint_soft_circle(pix, w, ph, x - 2.0, y + wing * 1.2, 1.1, col, 0.55);
    paint_soft_circle(pix, w, ph, x + 2.0, y - wing * 1.2, 1.1, col, 0.55);
}

fn paint_rect(pix: &mut Pix, w: usize, ph: usize, x: f64, y: f64, rw: f64, rh: f64, col: Rgb, power: f64) {
    let min_x = x.floor() as i32;
    let max_x = (x + rw).ceil() as i32;
    let min_y = y.floor() as i32;
    let max_y = (y + rh).ceil() as i32;

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            if in_bounds(w, ph, px, py) {
                pix[py as usize][px as usize] =
                    blend(pix[py as usize][px as usize], col, power);
            }
        }
    }
}

fn paint_soft_circle(pix: &mut Pix, w: usize, ph: usize, cx: f64, cy: f64, r: f64, col: Rgb, power: f64) {
    let min_x = (cx - r - 1.0).floor() as i32;
    let max_x = (cx + r + 1.0).ceil() as i32;
    let min_y = (cy - r - 1.0).floor() as i32;
    let max_y = (cy + r + 1.0).ceil() as i32;

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            if !in_bounds(w, ph, px, py) {
                continue;
            }

            let dx = px as f64 - cx;
            let dy = py as f64 - cy;
            let d = (dx * dx + dy * dy).sqrt();

            if d <= r {
                let a = (1.0 - d / r.max(1.0)).powf(1.35) * power;
                pix[py as usize][px as usize] =
                    blend(pix[py as usize][px as usize], col, a);
            }
        }
    }
}

fn in_bounds(w: usize, ph: usize, x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < w as i32 && y < ph as i32
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
