// ===== src/modes/neon_subway.rs =====
//
// NeonSubwayMode
//
// A terminal-native subway / cyber tunnel mode.
// This one intentionally uses ASCII/Unicode glyphs more aggressively instead
// of only smooth half-block painting.
//
// Scene concept:
//   - perspective tunnel walls
//   - moving track ties
//   - animated signal lights
//   - side advertisements
//   - sparks on the rail
//   - occasional passing train
//   - neon rain/leaks
//
// The goal is a readable "terminal scene" with motion, not just a pixel painting.

use crate::ansi::{bg_rgb, rgb, RESET};
use crate::color::ColorProvider;
use crate::mode_base::Mode;
use rand::RngExt;

type Rgb = (u8, u8, u8);

#[derive(Clone, Copy)]
struct Spark {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    life: f64,
    max_life: f64,
    color: Rgb,
}

#[derive(Clone, Copy)]
struct Drip {
    x: f64,
    y: f64,
    speed: f64,
    color: Rgb,
}

#[derive(Clone, Copy)]
struct AdPanel {
    side: i32,
    y: f64,
    phase: f64,
    color: Rgb,
}

#[derive(Clone, Copy)]
struct Train {
    active: bool,
    z: f64,
    cooldown: f64,
    side_offset: f64,
}

pub struct NeonSubwayMode {
    speed: f64,
    color_provider: ColorProvider,
    time: f64,
    sparks: Vec<Spark>,
    drips: Vec<Drip>,
    ads: Vec<AdPanel>,
    train: Train,
}

impl NeonSubwayMode {
    pub fn new(speed: f64, color_provider: ColorProvider) -> Self {
        let mut rng = rand::rng();

        let drips = (0..42)
            .map(|_| Drip {
                x: rng.random_range(0.0..160.0),
                y: rng.random_range(0.0..80.0),
                speed: rng.random_range(3.0..12.0),
                color: if rng.random_range(0..2) == 0 {
                    (65, 225, 255)
                } else {
                    (255, 80, 190)
                },
            })
            .collect();

        let ads = (0..10)
            .map(|i| AdPanel {
                side: if i % 2 == 0 { -1 } else { 1 },
                y: rng.random_range(0.15..0.78),
                phase: rng.random_range(0.0..std::f64::consts::TAU),
                color: match i % 4 {
                    0 => (255, 80, 180),
                    1 => (90, 230, 255),
                    2 => (255, 210, 80),
                    _ => (140, 255, 140),
                },
            })
            .collect();

        Self {
            speed,
            color_provider,
            time: 0.0,
            sparks: Vec::new(),
            drips,
            ads,
            train: Train {
                active: false,
                z: 0.0,
                cooldown: 5.0,
                side_offset: 0.0,
            },
        }
    }
}

impl Mode for NeonSubwayMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        let dt = dt * self.speed;
        self.time += dt;

        let w = width.max(1) as f64;
        let h = height.max(1) as f64;

        let mut rng = rand::rng();

        for drip in &mut self.drips {
            drip.y += drip.speed * dt;
            if drip.y > h {
                drip.y = -rng.random_range(0.0..8.0);
                drip.x = rng.random_range(0.0..w);
            }
        }

        if rng.random_range(0.0..1.0) < 0.35 {
            let rail_side = if rng.random_range(0..2) == 0 { -1.0 } else { 1.0 };
            let cx = w * 0.5 + rail_side * w * 0.11 + rng.random_range(-1.0..1.0);
            let cy = h * 0.78 + rng.random_range(-1.0..2.0);
            self.sparks.push(Spark {
                x: cx,
                y: cy,
                vx: rng.random_range(-6.0..6.0),
                vy: rng.random_range(-8.0..-2.0),
                life: rng.random_range(0.18..0.55),
                max_life: 0.55,
                color: if rng.random_range(0..2) == 0 {
                    (255, 200, 90)
                } else {
                    (110, 230, 255)
                },
            });
        }

        for spark in &mut self.sparks {
            spark.x += spark.vx * dt;
            spark.y += spark.vy * dt;
            spark.vy += 18.0 * dt;
            spark.life -= dt;
        }

        self.sparks.retain(|s| {
            s.life > 0.0 && s.x > -4.0 && s.x < w + 4.0 && s.y > -4.0 && s.y < h + 4.0
        });

        if self.train.active {
            self.train.z += dt * 1.25;
            if self.train.z > 1.35 {
                self.train.active = false;
                self.train.cooldown = rng.random_range(8.0..18.0);
                self.train.z = 0.0;
            }
        } else {
            self.train.cooldown -= dt;
            if self.train.cooldown <= 0.0 {
                self.train.active = true;
                self.train.z = 0.02;
                self.train.side_offset = rng.random_range(-0.08..0.08);
            }
        }

        if self.sparks.len() > 140 {
            let drop = self.sparks.len() - 140;
            self.sparks.drain(..drop);
        }
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w = width.max(1) as usize;
        let h = height.max(1) as usize;

        let mut cells = vec![
            Cell {
                ch: ' ',
                fg: (120, 130, 150),
                bg: (3, 5, 12),
            };
            w * h
        ];

        paint_background(&mut cells, w, h, self.time);
        paint_tunnel(&mut cells, w, h, self.time);
        paint_ads(&mut cells, w, h, &self.ads, self.time);
        paint_drips(&mut cells, w, h, &self.drips, self.time);
        paint_tracks(&mut cells, w, h, self.time);
        paint_signals(&mut cells, w, h, self.time);

        if self.train.active {
            paint_train(&mut cells, w, h, self.train, self.time);
        }

        for spark in &self.sparks {
            paint_spark(&mut cells, w, h, spark);
        }

        compose(&cells, w, h, &self.color_provider, t_abs)
    }
}

#[derive(Clone, Copy)]
struct Cell {
    ch: char,
    fg: Rgb,
    bg: Rgb,
}

fn idx(w: usize, x: i32, y: i32) -> Option<usize> {
    if x < 0 || y < 0 {
        return None;
    }

    let x = x as usize;
    let y = y as usize;

    Some(y * w + x)
}

fn set(cells: &mut [Cell], w: usize, h: usize, x: i32, y: i32, ch: char, fg: Rgb, bg: Option<Rgb>) {
    if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
        return;
    }

    let i = y as usize * w + x as usize;
    cells[i].ch = ch;
    cells[i].fg = fg;
    if let Some(bg) = bg {
        cells[i].bg = bg;
    }
}

fn blend(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let c = |av: u8, bv: u8| (av as f64 + (bv as f64 - av as f64) * t) as u8;
    (c(a.0, b.0), c(a.1, b.1), c(a.2, b.2))
}

fn darken(c: Rgb, f: f64) -> Rgb {
    let s = (1.0 - f).clamp(0.0, 1.0);
    (
        (c.0 as f64 * s) as u8,
        (c.1 as f64 * s) as u8,
        (c.2 as f64 * s) as u8,
    )
}

fn paint_background(cells: &mut [Cell], w: usize, h: usize, time: f64) {
    for y in 0..h {
        let yf = y as f64 / h.max(1) as f64;
        let bg = blend((3, 5, 12), (14, 8, 28), yf);

        for x in 0..w {
            let scan = if y % 4 == 0 { 0.08 } else { 0.0 };
            let pulse = ((time * 0.6 + x as f64 * 0.03).sin() + 1.0) * 0.5;
            cells[y * w + x].bg = blend(bg, (30, 16, 60), pulse * 0.05 + scan);
            cells[y * w + x].fg = (55, 65, 85);
            cells[y * w + x].ch = ' ';
        }
    }
}

fn tunnel_bounds(w: usize, h: usize, y: usize) -> (i32, i32) {
    let yf = y as f64 / h.max(1) as f64;
    let center = w as f64 * 0.5;
    let half = 2.0 + yf.powf(1.65) * w as f64 * 0.47;
    ((center - half) as i32, (center + half) as i32)
}

fn paint_tunnel(cells: &mut [Cell], w: usize, h: usize, time: f64) {
    let van_y = (h as f64 * 0.20) as i32;
    let center = w as i32 / 2;

    for y in 0..h {
        let (l, r) = tunnel_bounds(w, h, y);
        let yf = y as f64 / h.max(1) as f64;

        let wall_col = blend((18, 20, 34), (40, 35, 58), yf);
        for x in 0..w as i32 {
            if x < l || x > r {
                if let Some(i) = idx(w, x, y as i32) {
                    cells[i].bg = darken(wall_col, 0.2);
                    cells[i].ch = if (x + y as i32) % 7 == 0 { '·' } else { ' ' };
                    cells[i].fg = (70, 80, 100);
                }
            }
        }

        let edge_col = if ((time * 5.0 + y as f64 * 0.2).sin()) > 0.7 {
            (120, 235, 255)
        } else {
            (80, 130, 180)
        };

        set(cells, w, h, l, y as i32, '╱', edge_col, None);
        set(cells, w, h, r, y as i32, '╲', edge_col, None);
    }

    // Ceiling perspective ribs.
    for k in 0..14 {
        let t = ((time * 0.7 + k as f64 * 0.14).rem_euclid(1.0)).powf(1.8);
        let y = van_y + (t * (h as f64 - van_y as f64)) as i32;
        let (l, r) = tunnel_bounds(w, h, y.clamp(0, h as i32 - 1) as usize);
        let col = blend((40, 80, 120), (160, 230, 255), t);
        for x in l..=r {
            if (x - center).abs() % 3 == 0 {
                set(cells, w, h, x, y, '─', col, None);
            }
        }
    }
}

fn paint_tracks(cells: &mut [Cell], w: usize, h: usize, time: f64) {
    let center = w as i32 / 2;
    let rail_col = (160, 170, 185);
    let glow = (95, 205, 255);

    for y in 0..h as i32 {
        let yf = y as f64 / h.max(1) as f64;
        if yf < 0.30 {
            continue;
        }

        let spread = (yf.powf(1.8) * w as f64 * 0.14) as i32;
        let lx = center - spread;
        let rx = center + spread;

        set(cells, w, h, lx, y, '╱', rail_col, None);
        set(cells, w, h, rx, y, '╲', rail_col, None);

        if y % 3 == 0 {
            set(cells, w, h, lx + 1, y, '·', glow, None);
            set(cells, w, h, rx - 1, y, '·', glow, None);
        }
    }

    // Track ties moving toward the viewer.
    for k in 0..18 {
        let t = ((time * 0.95 + k as f64 * 0.085).rem_euclid(1.0)).powf(1.75);
        let y = (h as f64 * 0.30 + t * h as f64 * 0.70) as i32;
        let len = (t * w as f64 * 0.34) as i32;
        let col = blend((55, 48, 58), (145, 110, 130), t);
        for x in center - len..=center + len {
            if x % 2 == 0 {
                set(cells, w, h, x, y, '═', col, None);
            }
        }
    }
}

fn paint_ads(cells: &mut [Cell], w: usize, h: usize, ads: &[AdPanel], time: f64) {
    for ad in ads {
        let y = (ad.y * h as f64) as i32;
        let width = (w as f64 * 0.12).clamp(8.0, 18.0) as i32;
        let height = 3;
        let x = if ad.side < 0 {
            (w as f64 * 0.08) as i32
        } else {
            (w as f64 * 0.80) as i32
        };

        let blink = ((time * 2.5 + ad.phase).sin() + 1.0) * 0.5;
        let fg = blend(ad.color, (255, 255, 255), blink * 0.35);
        let bg = darken(ad.color, 0.72);

        for dy in 0..height {
            for dx in 0..width {
                let border = dy == 0 || dy == height - 1 || dx == 0 || dx == width - 1;
                let ch = if border {
                    if dy == 0 || dy == height - 1 { '═' } else { '║' }
                } else if (dx + y + dy) % 5 == 0 {
                    '#'
                } else if (dx + dy) % 3 == 0 {
                    '+'
                } else {
                    ' '
                };

                set(cells, w, h, x + dx, y + dy, ch, fg, Some(bg));
            }
        }
    }
}

fn paint_signals(cells: &mut [Cell], w: usize, h: usize, time: f64) {
    let center = w as i32 / 2;
    let y = (h as f64 * 0.24) as i32;
    let blink = ((time * 3.5).sin() + 1.0) * 0.5;

    let red = if blink > 0.5 { (255, 45, 60) } else { (90, 25, 35) };
    let cyan = if blink <= 0.5 { (80, 235, 255) } else { (30, 80, 100) };

    set(cells, w, h, center - 6, y, '●', red, None);
    set(cells, w, h, center + 6, y, '●', cyan, None);
    set(cells, w, h, center - 7, y, '[', (140, 150, 165), None);
    set(cells, w, h, center + 7, y, ']', (140, 150, 165), None);
}

fn paint_drips(cells: &mut [Cell], w: usize, h: usize, drips: &[Drip], time: f64) {
    for d in drips {
        let x = (d.x + (time * 0.8 + d.y * 0.13).sin()).round() as i32;
        let y = d.y.round() as i32;

        let ch = if (d.y as i32) % 3 == 0 { '│' } else { '╷' };
        set(cells, w, h, x, y, ch, d.color, None);
    }
}

fn paint_spark(cells: &mut [Cell], w: usize, h: usize, s: &Spark) {
    let fade = (s.life / s.max_life).clamp(0.0, 1.0);
    let col = blend((40, 35, 40), s.color, fade);
    let ch = if fade > 0.66 { '*' } else if fade > 0.33 { '+' } else { '·' };
    set(cells, w, h, s.x.round() as i32, s.y.round() as i32, ch, col, None);
}

fn paint_train(cells: &mut [Cell], w: usize, h: usize, train: Train, time: f64) {
    let t = train.z.clamp(0.0, 1.35);
    let center = w as f64 * (0.5 + train.side_offset);
    let y = h as f64 * (0.18 + t * 0.55);
    let half_w = (w as f64 * 0.05 + t * w as f64 * 0.28) as i32;
    let half_h = (2.0 + t * h as f64 * 0.18) as i32;

    let cx = center.round() as i32;
    let cy = y.round() as i32;

    let body = blend((35, 38, 52), (95, 100, 120), t * 0.65);
    let edge = (170, 210, 230);

    for dy in -half_h..=half_h {
        for dx in -half_w..=half_w {
            let x = cx + dx;
            let y = cy + dy;

            let border = dy.abs() == half_h || dx.abs() == half_w;
            let window = dy.abs() < half_h / 2 && dx.abs() < half_w - 2 && dx % 5 == 0;
            let nose = dy == -half_h / 2 && dx.abs() < half_w / 2;

            let ch = if border {
                if dy.abs() == half_h { '═' } else { '║' }
            } else if window {
                '▣'
            } else if nose {
                '─'
            } else {
                ' '
            };

            let fg = if border {
                edge
            } else if window {
                (255, 230, 120)
            } else {
                body
            };

            set(cells, w, h, x, y, ch, fg, Some(darken(body, 0.35)));
        }
    }

    let pulse = ((time * 8.0).sin() + 1.0) * 0.5;
    let light = blend((255, 200, 100), (255, 255, 220), pulse);
    set(cells, w, h, cx - half_w / 2, cy + half_h / 2, '●', light, None);
    set(cells, w, h, cx + half_w / 2, cy + half_h / 2, '●', light, None);
}

fn compose(cells: &[Cell], w: usize, h: usize, color_provider: &ColorProvider, t_abs: f64) -> String {
    let mut out = String::with_capacity(w * h * 24);
    let mut last_fg: Option<Rgb> = None;
    let mut last_bg: Option<Rgb> = None;

    for y in 0..h {
        for x in 0..w {
            let cell = cells[y * w + x];

            let fg = color_provider.tint(cell.fg, t_abs, x as i32, y as i32);
            let bg = color_provider.tint(cell.bg, t_abs, x as i32, y as i32);

            if Some(fg) != last_fg {
                out.push_str(&rgb(fg.0, fg.1, fg.2));
                last_fg = Some(fg);
            }

            if Some(bg) != last_bg {
                out.push_str(&bg_rgb(bg.0, bg.1, bg.2));
                last_bg = Some(bg);
            }

            out.push(cell.ch);
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
