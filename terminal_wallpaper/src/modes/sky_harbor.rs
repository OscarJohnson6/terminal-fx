// ===== src/modes/sky_harbor.rs =====
//
// SkyHarborMode
//
// A high-detail floating-island / airship harbor simulation.
// Inspired by the existing LandscapeMode half-block renderer and CastleSiegeMode's
// entity/timer-driven simulation style.
//
// SCENE LAYERS:
//   sky gradient -> stars -> distant storm clouds -> parallax clouds ->
//   floating islands -> waterfalls -> airships -> beacon lights -> lightning flash
//
// RENDERING:
//   Uses the same high-resolution half-block trick as LandscapeMode:
//   each terminal row encodes two pixel rows using '▀' with foreground/background
//   RGB colors. This gives double vertical resolution in a normal terminal.

use crate::ansi::{bg_rgb, rgb, RESET};
use crate::color::ColorProvider;
use crate::mode_base::Mode;
use rand::RngExt;
use std::f64::consts::TAU;

type Rgb = (u8, u8, u8);
type Pix = Vec<Vec<Rgb>>;

#[derive(Clone, Copy)]
struct Island {
    x_frac: f64,
    y_frac: f64,
    radius_x: f64,
    radius_y: f64,
    seed: f64,
    drift_phase: f64,
    beacon_phase: f64,
    waterfall_offset: f64,
}

#[derive(Clone, Copy)]
struct Cloud {
    x: f64,
    y_frac: f64,
    rx: f64,
    ry: f64,
    speed: f64,
    depth: f64,
}

#[derive(Clone, Copy)]
struct Airship {
    x: f64,
    y: f64,
    speed: f64,
    size: f64,
    phase: f64,
    lane: f64,
    direction: f64,
    blink_phase: f64,
}

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
struct StormCell {
    x_frac: f64,
    y_frac: f64,
    intensity: f64,
    timer: f64,
    cooldown: f64,
}

pub struct SkyHarborMode {
    speed: f64,
    color_provider: ColorProvider,
    time: f64,
    wind: f64,
    islands: Vec<Island>,
    clouds: Vec<Cloud>,
    airships: Vec<Airship>,
    sparks: Vec<Spark>,
    storms: Vec<StormCell>,
}

impl SkyHarborMode {
    pub fn new(speed: f64, color_provider: ColorProvider) -> Self {
        let mut rng = rand::rng();

        let islands = vec![
            Island {
                x_frac: 0.20,
                y_frac: 0.62,
                radius_x: 22.0,
                radius_y: 10.0,
                seed: rng.random_range(0.0..1000.0),
                drift_phase: rng.random_range(0.0..TAU),
                beacon_phase: rng.random_range(0.0..TAU),
                waterfall_offset: rng.random_range(0.0..TAU),
            },
            Island {
                x_frac: 0.50,
                y_frac: 0.47,
                radius_x: 30.0,
                radius_y: 13.0,
                seed: rng.random_range(0.0..1000.0),
                drift_phase: rng.random_range(0.0..TAU),
                beacon_phase: rng.random_range(0.0..TAU),
                waterfall_offset: rng.random_range(0.0..TAU),
            },
            Island {
                x_frac: 0.78,
                y_frac: 0.66,
                radius_x: 24.0,
                radius_y: 11.0,
                seed: rng.random_range(0.0..1000.0),
                drift_phase: rng.random_range(0.0..TAU),
                beacon_phase: rng.random_range(0.0..TAU),
                waterfall_offset: rng.random_range(0.0..TAU),
            },
        ];

        let clouds = (0..12)
            .map(|_| Cloud {
                x: rng.random_range(-120.0..260.0),
                y_frac: rng.random_range(0.05..0.52),
                rx: rng.random_range(16.0..48.0),
                ry: rng.random_range(3.0..9.0),
                speed: rng.random_range(0.8..4.2),
                depth: rng.random_range(0.25..1.0),
            })
            .collect();

        let airships = (0..7)
            .map(|i| {
                let direction = if i % 2 == 0 { 1.0 } else { -1.0 };
                Airship {
                    x: rng.random_range(-100.0..260.0),
                    y: rng.random_range(12.0..55.0),
                    speed: rng.random_range(3.5..10.0),
                    size: rng.random_range(0.85..1.35),
                    phase: rng.random_range(0.0..TAU),
                    lane: rng.random_range(0.0..1.0),
                    direction,
                    blink_phase: rng.random_range(0.0..TAU),
                }
            })
            .collect();

        let storms = vec![
            StormCell {
                x_frac: 0.12,
                y_frac: 0.20,
                intensity: 0.0,
                timer: 0.0,
                cooldown: rng.random_range(3.0..8.0),
            },
            StormCell {
                x_frac: 0.88,
                y_frac: 0.28,
                intensity: 0.0,
                timer: 0.0,
                cooldown: rng.random_range(5.0..10.0),
            },
        ];

        Self {
            speed,
            color_provider,
            time: 0.0,
            wind: 0.0,
            islands,
            clouds,
            airships,
            sparks: Vec::new(),
            storms,
        }
    }
}

impl Mode for SkyHarborMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        let dt = dt * self.speed;
        let w = width.max(1) as f64;
        let h = height.max(1) as f64 * 2.0;

        self.time += dt;
        self.wind += dt * (0.35 + 0.18 * (self.time * 0.07).sin());

        for cloud in &mut self.clouds {
            cloud.x += cloud.speed * cloud.depth * dt;
            if cloud.x > w + cloud.rx * 2.0 {
                cloud.x = -cloud.rx * 2.0;
            }
        }

        for ship in &mut self.airships {
            ship.x += ship.speed * ship.direction * dt;
            ship.phase += dt * 1.3;
            ship.blink_phase += dt * 5.0;

            ship.y += (ship.phase * 0.9 + ship.lane * 7.0).sin() * dt * 0.9;

            let margin = 80.0 * ship.size;
            if ship.direction > 0.0 && ship.x > w + margin {
                ship.x = -margin;
                ship.y = 12.0 + ship.lane * h * 0.28;
            } else if ship.direction < 0.0 && ship.x < -margin {
                ship.x = w + margin;
                ship.y = 12.0 + ship.lane * h * 0.28;
            }
        }

        let mut rng = rand::rng();

        for storm in &mut self.storms {
            if storm.timer > 0.0 {
                storm.timer -= dt;
                storm.intensity = (storm.timer / 0.22).clamp(0.0, 1.0);
            } else {
                storm.intensity = 0.0;
                storm.cooldown -= dt;
                if storm.cooldown <= 0.0 {
                    storm.timer = rng.random_range(0.08..0.22);
                    storm.cooldown = rng.random_range(4.0..11.0);
                }
            }
        }

        // Small glowing engine sparks from ships.
        for ship in &self.airships {
            if rng.random_range(0.0..1.0) < 0.18 * self.speed.min(3.0) {
                let tail_x = ship.x - ship.direction * 8.0 * ship.size;
                let tail_y = ship.y + 1.5 * ship.size;
                self.sparks.push(Spark {
                    x: tail_x,
                    y: tail_y,
                    vx: -ship.direction * rng.random_range(3.0..9.0),
                    vy: rng.random_range(-1.0..2.5),
                    life: rng.random_range(0.28..0.65),
                    max_life: 0.65,
                    color: (255, 170, 70),
                });
            }
        }

        for spark in &mut self.sparks {
            spark.x += spark.vx * dt;
            spark.y += spark.vy * dt;
            spark.vy += 4.0 * dt;
            spark.life -= dt;
        }

        self.sparks.retain(|s| {
            s.life > 0.0 && s.x > -5.0 && s.x < w + 5.0 && s.y > -5.0 && s.y < h + 5.0
        });

        if self.sparks.len() > 220 {
            let drop = self.sparks.len() - 220;
            self.sparks.drain(..drop);
        }
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w = width.max(1) as usize;
        let h = height.max(1) as usize;
        let ph = h * 2;

        let mut pix: Pix = vec![vec![(0, 0, 0); w]; ph];

        let cycle = (self.time * 0.0025).rem_euclid(1.0);

        paint_sky(&mut pix, w, ph, cycle, self.time);
        paint_stars(&mut pix, w, ph, cycle, self.time);
        paint_storms(&mut pix, w, ph, &self.storms, cycle, self.time);

        for cloud in &self.clouds {
            paint_cloud(&mut pix, w, ph, cloud, cycle);
        }

        for island in &self.islands {
            paint_island(&mut pix, w, ph, island, cycle, self.time);
        }

        for ship in &self.airships {
            paint_airship(&mut pix, w, ph, ship, cycle, self.time);
        }

        for spark in &self.sparks {
            paint_spark(&mut pix, w, ph, spark);
        }

        paint_lightning_overlay(&mut pix, w, ph, &self.storms);

        half_blocks(&pix, w, h, &self.color_provider, t_abs)
    }
}

// ── Color helpers ─────────────────────────────────────────────────────────────

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

fn cycle4(t: f64, dawn: Rgb, day: Rgb, dusk: Rgb, night: Rgb) -> Rgb {
    match t {
        t if t < 0.25 => lerp(dawn, day, t / 0.25),
        t if t < 0.50 => lerp(day, dusk, (t - 0.25) / 0.25),
        t if t < 0.75 => lerp(dusk, night, (t - 0.50) / 0.25),
        t => lerp(night, dawn, (t - 0.75) / 0.25),
    }
}

fn sky_top(t: f64) -> Rgb {
    cycle4(t, (68, 42, 100), (28, 105, 190), (105, 45, 70), (4, 8, 28))
}

fn sky_bottom(t: f64) -> Rgb {
    cycle4(t, (235, 135, 90), (120, 190, 235), (235, 95, 55), (15, 20, 55))
}

fn cloud_color(t: f64) -> Rgb {
    cycle4(t, (235, 185, 160), (238, 242, 250), (255, 150, 95), (62, 70, 98))
}

fn island_grass(t: f64) -> Rgb {
    cycle4(t, (55, 95, 50), (60, 130, 45), (72, 92, 38), (14, 38, 16))
}

fn island_rock(t: f64) -> Rgb {
    cycle4(t, (76, 60, 72), (95, 87, 78), (75, 55, 58), (24, 24, 38))
}

fn water_color(t: f64) -> Rgb {
    cycle4(t, (95, 175, 205), (75, 190, 230), (115, 150, 220), (45, 90, 150))
}

// ── Painting helpers ──────────────────────────────────────────────────────────

fn paint_sky(pix: &mut Pix, w: usize, ph: usize, cycle: f64, time: f64) {
    for y in 0..ph {
        let yf = y as f64 / ph.max(1) as f64;
        let mut col = lerp(sky_top(cycle), sky_bottom(cycle), yf.powf(0.72));

        // Subtle aurora / high atmosphere movement.
        let aurora = ((y as f64 * 0.055 + time * 0.4).sin()
            + (time * 0.25 + y as f64 * 0.027).cos())
            * 0.5;
        if y < ph / 2 && aurora > 0.45 {
            col = blend(col, (50, 210, 180), (aurora - 0.45) * 0.16);
        }

        for x in 0..w {
            pix[y][x] = col;
        }
    }
}

fn paint_stars(pix: &mut Pix, w: usize, ph: usize, cycle: f64, time: f64) {
    let night = if cycle < 0.10 {
        1.0 - cycle / 0.10
    } else if cycle < 0.48 {
        0.0
    } else if cycle < 0.62 {
        (cycle - 0.48) / 0.14
    } else {
        1.0
    };

    if night <= 0.01 {
        return;
    }

    for i in 0..360usize {
        let sx = (i.wrapping_mul(2654435761) >> 4) % w.max(1);
        let sy = (i.wrapping_mul(2246822519) >> 4) % (ph / 2).max(1);
        let twinkle = ((time * 2.1 + i as f64 * 0.73).sin() + 1.0) * 0.5;
        let b = (twinkle * night * 235.0) as u8;
        if b > 25 {
            pix[sy][sx] = (b, b, (b as f64 * 0.92) as u8);
        }
    }
}

fn paint_storms(pix: &mut Pix, w: usize, ph: usize, storms: &[StormCell], cycle: f64, time: f64) {
    let base = darken(cloud_color(cycle), 0.35);

    for storm in storms {
        let cx = (storm.x_frac * w as f64) as i32;
        let cy = (storm.y_frac * ph as f64) as i32;
        let rx = (w as f64 * 0.22) as i32;
        let ry = (ph as f64 * 0.10) as i32;

        for dy in -ry..=ry {
            for dx in -rx..=rx {
                let ex = dx as f64 / rx.max(1) as f64;
                let ey = dy as f64 / ry.max(1) as f64;
                let d = ex * ex + ey * ey;

                if d <= 1.0 {
                    let px = cx + dx;
                    let py = cy + dy;

                    if px >= 0 && px < w as i32 && py >= 0 && py < ph as i32 {
                        let rumble = ((dx as f64 * 0.12 + time * 0.8).sin() + 1.0) * 0.5;
                        let alpha = (1.0 - d).powf(0.8) * (0.30 + rumble * 0.20);
                        let mut col = blend(pix[py as usize][px as usize], base, alpha);

                        if storm.intensity > 0.0 {
                            col = blend(col, (220, 215, 255), storm.intensity * 0.35);
                        }

                        pix[py as usize][px as usize] = col;
                    }
                }
            }
        }
    }
}

fn paint_cloud(pix: &mut Pix, w: usize, ph: usize, cloud: &Cloud, cycle: f64) {
    let c_col = cloud_color(cycle);
    let cx = cloud.x as i32;
    let cy = (cloud.y_frac * ph as f64) as i32;
    let rx = cloud.rx as i32;
    let ry = cloud.ry as i32;

    for bump in -2..=2 {
        let bx = cx + bump * rx / 4;
        let by = cy - (bump.abs() % 2) * ry / 3;
        let brx = (rx as f64 * (0.48 + 0.10 * (bump as f64).abs())) as i32;
        let bry = (ry as f64 * (0.75 + 0.08 * (bump as f64).abs())) as i32;

        for dy in -bry..=bry {
            for dx in -brx..=brx {
                let ex = dx as f64 / brx.max(1) as f64;
                let ey = dy as f64 / bry.max(1) as f64;
                let d = ex * ex + ey * ey;

                if d <= 1.0 {
                    let px = bx + dx;
                    let py = by + dy;
                    if px >= 0 && px < w as i32 && py >= 0 && py < ph as i32 {
                        let alpha = (1.0 - d).powf(0.55) * 0.58 * cloud.depth;
                        pix[py as usize][px as usize] =
                            blend(pix[py as usize][px as usize], c_col, alpha);
                    }
                }
            }
        }
    }
}

fn paint_island(pix: &mut Pix, w: usize, ph: usize, island: &Island, cycle: f64, time: f64) {
    let cx = (island.x_frac * w as f64) as i32;
    let drift_y = (time * 0.45 + island.drift_phase).sin() * 1.7;
    let cy = (island.y_frac * ph as f64 + drift_y) as i32;

    let rx = island.radius_x.min(w as f64 * 0.34) as i32;
    let ry = island.radius_y.min(ph as f64 * 0.22) as i32;

    let grass = island_grass(cycle);
    let rock = island_rock(cycle);
    let water = water_color(cycle);

    // Rock body / floating underside.
    for dy in -ry..=(ry * 2) {
        for dx in -rx..=rx {
            let ex = dx as f64 / rx.max(1) as f64;
            let ey = if dy < 0 {
                dy as f64 / ry.max(1) as f64
            } else {
                dy as f64 / (ry * 2).max(1) as f64
            };

            let body = ex * ex + ey * ey;
            let taper = 1.0 - (dy.max(0) as f64 / (ry * 2).max(1) as f64) * 0.72;

            if body <= taper.max(0.18) {
                let px = cx + dx;
                let py = cy + dy;

                if px >= 0 && px < w as i32 && py >= 0 && py < ph as i32 {
                    let depth = (dy + ry) as f64 / (ry * 3).max(1) as f64;
                    let noise = fbm(dx as f64 * 1.9, island.seed + py as f64 * 0.15);
                    let mut col = if dy < -ry / 3 {
                        blend(grass, brighten(grass, 25), noise.abs() * 0.28)
                    } else {
                        blend(rock, darken(rock, 0.45), depth)
                    };

                    // Bright grassy rim.
                    if dy.abs() <= 1 && ex.abs() < 0.94 {
                        col = brighten(grass, 25);
                    }

                    // Small mineral veins underneath.
                    if dy > ry / 2 && noise > 0.72 {
                        col = blend(col, (90, 180, 190), 0.45);
                    }

                    pix[py as usize][px as usize] = col;
                }
            }
        }
    }

    // Tiny buildings / towers on top.
    let tower_count = 3;
    for i in 0..tower_count {
        let tx = cx - rx / 2 + i * rx / 2;
        let tower_h = 5 + ((island.seed as i32 + i * 7).abs() % 7);
        let tower_w = 2 + (i % 2);

        for ty in 0..tower_h {
            for tw in 0..tower_w {
                let px = tx + tw;
                let py = cy - ry / 2 - ty;
                if px >= 0 && px < w as i32 && py >= 0 && py < ph as i32 {
                    let col = if ty == tower_h - 1 {
                        (190, 150, 95)
                    } else {
                        (84, 74, 88)
                    };
                    pix[py as usize][px as usize] = blend(pix[py as usize][px as usize], col, 0.94);
                }
            }
        }
    }

    // Beacon light.
    let bx = cx + rx / 3;
    let by = cy - ry / 2 - 9;
    let pulse = ((time * 4.0 + island.beacon_phase).sin() + 1.0) * 0.5;
    paint_glow(pix, w, ph, bx, by, 5, (255, 190, 85), 0.15 + pulse * 0.55);

    // Waterfall from one island edge.
    let wx = cx - rx / 4;
    let top = cy;
    let len = (ph as f64 * 0.20) as i32;
    for k in 0..len {
        let py = top + k;
        if py < 0 || py >= ph as i32 {
            continue;
        }

        let wiggle = ((k as f64 * 0.45 + time * 2.5 + island.waterfall_offset).sin() * 1.6) as i32;
        let px = wx + wiggle;

        if px >= 0 && px < w as i32 {
            let fade = 1.0 - k as f64 / len.max(1) as f64;
            let shimmer = ((time * 7.0 + k as f64 * 0.3).sin() + 1.0) * 0.5;
            let col = blend(water, (220, 245, 255), shimmer * 0.35);
            pix[py as usize][px as usize] =
                blend(pix[py as usize][px as usize], col, 0.30 + fade * 0.65);
        }
    }
}

fn paint_airship(pix: &mut Pix, w: usize, ph: usize, ship: &Airship, cycle: f64, time: f64) {
    let x = ship.x.round() as i32;
    let y = ship.y.round() as i32;
    let dir = ship.direction.signum() as i32;
    let s = ship.size;

    let hull = cycle4(cycle, (130, 82, 75), (145, 95, 70), (120, 70, 80), (80, 55, 72));
    let balloon = cycle4(cycle, (180, 130, 105), (205, 170, 120), (185, 100, 95), (95, 75, 105));
    let cabin = (80, 65, 55);

    let brx = (10.0 * s) as i32;
    let bry = (3.5 * s).max(2.0) as i32;

    // Balloon ellipse.
    for dy in -bry..=bry {
        for dx in -brx..=brx {
            let ex = dx as f64 / brx.max(1) as f64;
            let ey = dy as f64 / bry.max(1) as f64;
            let d = ex * ex + ey * ey;

            if d <= 1.0 {
                let px = x + dx;
                let py = y + dy;
                if px >= 0 && px < w as i32 && py >= 0 && py < ph as i32 {
                    let shade = 0.18 * (dx as f64 / brx.max(1) as f64);
                    let col = if shade > 0.0 {
                        brighten(balloon, (shade * 40.0) as i32)
                    } else {
                        darken(balloon, -shade)
                    };
                    pix[py as usize][px as usize] = blend(pix[py as usize][px as usize], col, 0.96);
                }
            }
        }
    }

    // Hull.
    let hull_y = y + (5.0 * s) as i32;
    for dy in 0..=(2.0 * s) as i32 {
        for dx in -(7.0 * s) as i32..=(7.0 * s) as i32 {
            let px = x + dx;
            let py = hull_y + dy;

            if px >= 0 && px < w as i32 && py >= 0 && py < ph as i32 {
                let edge = dx.abs() as f64 / (7.0 * s).max(1.0);
                if edge < 1.0 {
                    let col = blend(hull, darken(hull, 0.35), edge * 0.65);
                    pix[py as usize][px as usize] = col;
                }
            }
        }
    }

    // Cabin.
    for dy in 0..3 {
        for dx in -2..=2 {
            let px = x + dx;
            let py = hull_y + dy;
            if px >= 0 && px < w as i32 && py >= 0 && py < ph as i32 {
                pix[py as usize][px as usize] = cabin;
            }
        }
    }

    // Ropes.
    for dx in [-5, -1, 3, 6] {
        let px = x + (dx as f64 * s) as i32;
        for py in y + bry..hull_y {
            if px >= 0 && px < w as i32 && py >= 0 && py < ph as i32 {
                pix[py as usize][px as usize] = blend(pix[py as usize][px as usize], (60, 45, 40), 0.8);
            }
        }
    }

    // Engine glow / tail.
    let glow_x = x - dir * (9.0 * s) as i32;
    let glow_y = hull_y + 1;
    let pulse = ((time * 9.0 + ship.blink_phase).sin() + 1.0) * 0.5;
    paint_glow(pix, w, ph, glow_x, glow_y, 4, (255, 135, 55), 0.25 + pulse * 0.45);

    // Nose signal light.
    let nose_x = x + dir * (8.0 * s) as i32;
    paint_glow(pix, w, ph, nose_x, hull_y, 3, (95, 200, 255), 0.20 + pulse * 0.35);
}

fn paint_spark(pix: &mut Pix, w: usize, ph: usize, spark: &Spark) {
    let x = spark.x.round() as i32;
    let y = spark.y.round() as i32;

    if x < 0 || x >= w as i32 || y < 0 || y >= ph as i32 {
        return;
    }

    let fade = (spark.life / spark.max_life).clamp(0.0, 1.0);
    let col = (
        (spark.color.0 as f64 * fade) as u8,
        (spark.color.1 as f64 * fade) as u8,
        (spark.color.2 as f64 * fade) as u8,
    );

    pix[y as usize][x as usize] = blend(pix[y as usize][x as usize], col, fade);
}

fn paint_lightning_overlay(pix: &mut Pix, w: usize, ph: usize, storms: &[StormCell]) {
    for storm in storms {
        if storm.intensity <= 0.0 {
            continue;
        }

        let sx = (storm.x_frac * w as f64) as i32;
        let mut x = sx;
        let mut y = (storm.y_frac * ph as f64) as i32;
        let target = (ph as f64 * 0.55) as i32;

        while y < target {
            if x >= 0 && x < w as i32 && y >= 0 && y < ph as i32 {
                pix[y as usize][x as usize] =
                    blend(pix[y as usize][x as usize], (240, 235, 255), storm.intensity);
            }

            let bend = ((y as f64 * 0.7 + sx as f64 * 0.21).sin() * 1.8).round() as i32;
            x += bend.clamp(-1, 1);
            y += 1;
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

fn fbm(x: f64, seed: f64) -> f64 {
    0.42 * (x * 0.034 + seed).sin()
        + 0.28 * (x * 0.081 + seed * 1.73).sin()
        + 0.16 * (x * 0.171 + seed * 0.91).sin()
        + 0.09 * (x * 0.317 + seed * 2.31).sin()
        + 0.05 * (x * 0.619 + seed * 0.53).sin()
}

// ── Half-block composer ───────────────────────────────────────────────────────

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
