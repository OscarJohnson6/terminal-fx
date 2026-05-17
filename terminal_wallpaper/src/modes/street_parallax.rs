// ===== src/modes/street_parallax.rs =====
//
// StreetParallaxMode
//
// Street-level side-scrolling city scene.
// You are watching from the sidewalk/street as buildings, signs, traffic,
// streetlights, windows, and reflections slide past.
//
// Visual concept:
//   - Far skyline moves slowly.
//   - Mid buildings move at medium speed.
//   - Foreground street/traffic moves faster.
//   - Cars pass in both lanes.
//   - Neon signs, traffic lights, window flicker, rain reflections.
//   - Theme changes the "film style" without overpowering the art.
//
// Suggested registry:
//   use crate::modes::street_parallax::StreetParallaxMode;
//   mode_builder!(build_street_parallax, StreetParallaxMode);
//
//   ModeEntry {
//       id: "street",
//       name: "Street View",
//       desc: "Side-scrolling city traffic",
//       fps: 50,
//       build: build_street_parallax,
//   }
//
// In src/modes/mod.rs:
//   pub mod street_parallax;

use crate::ansi::{bg_rgb, rgb, RESET};
use crate::color::{ColorMode, ColorProvider};
use crate::mode_base::Mode;
use rand::RngExt;

type Rgb = (u8, u8, u8);
type Pix = Vec<Vec<Rgb>>;

#[derive(Clone, Copy)]
enum SignKind {
    Open,
    Ramen,
    Hotel,
    Arcade,
    Cafe,
    Tech,
}

#[derive(Clone, Copy)]
struct Building {
    x: f64,
    w: f64,
    h: f64,
    depth: f64,
    color: Rgb,
    window_color: Rgb,
    sign: Option<SignKind>,
    seed: f64,
}

#[derive(Clone, Copy)]
enum VehicleKind {
    Sedan,
    Taxi,
    Bus,
    Van,
    Sports,
}

#[derive(Clone, Copy)]
struct Vehicle {
    x: f64,
    lane: usize,
    speed: f64,
    length: f64,
    color: Rgb,
    kind: VehicleKind,
    going_right: bool,
    seed: f64,
}

#[derive(Clone, Copy)]
struct Pedestrian {
    x: f64,
    y: f64,
    speed: f64,
    color: Rgb,
    phase: f64,
}

#[derive(Clone, Copy)]
struct Particle {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    life: f64,
    max_life: f64,
    color: Rgb,
    kind: ParticleKind,
}

#[derive(Clone, Copy)]
enum ParticleKind {
    Rain,
    Exhaust,
    Spark,
    Reflection,
}

pub struct StreetParallaxMode {
    speed: f64,
    color_provider: ColorProvider,
    time: f64,
    camera: f64,

    far_buildings: Vec<Building>,
    mid_buildings: Vec<Building>,
    vehicles: Vec<Vehicle>,
    pedestrians: Vec<Pedestrian>,
    particles: Vec<Particle>,

    weather_rain: bool,
    last_dims: Option<(u16, u16)>,
}

impl StreetParallaxMode {
    pub fn new(speed: f64, color_provider: ColorProvider) -> Self {
        Self {
            speed,
            color_provider,
            time: 0.0,
            camera: 0.0,
            far_buildings: Vec::new(),
            mid_buildings: Vec::new(),
            vehicles: Vec::new(),
            pedestrians: Vec::new(),
            particles: Vec::new(),
            weather_rain: true,
            last_dims: None,
        }
    }

    fn reset_scene(&mut self, width: u16, height: u16) {
        let mut rng = rand::rng();
        let w = width.max(1) as f64;
        let ph = height.max(1) as f64 * 2.0;

        self.camera = 0.0;
        self.time = 0.0;
        self.far_buildings.clear();
        self.mid_buildings.clear();
        self.vehicles.clear();
        self.pedestrians.clear();
        self.particles.clear();

        self.weather_rain = match self.color_provider.mode {
            ColorMode::Ocean | ColorMode::Matrix => true,
            ColorMode::Sunset => rng.random_range(0.0..1.0) < 0.25,
            ColorMode::Rainbow => rng.random_range(0.0..1.0) < 0.45,
        };

        let far_total = w * 3.2;
        let mid_total = w * 3.0;

        let mut x = -w * 0.20;
        while x < far_total {
            let bw = rng.random_range(10.0..26.0);
            let bh = rng.random_range(ph * 0.18..ph * 0.46);
            self.far_buildings.push(Building {
                x,
                w: bw,
                h: bh,
                depth: 0.28,
                color: random_building_color(&mut rng, true),
                window_color: random_window_color(&mut rng),
                sign: None,
                seed: rng.random_range(0.0..9999.0),
            });
            x += bw + rng.random_range(1.0..5.0);
        }

        x = -w * 0.15;
        while x < mid_total {
            let bw = rng.random_range(8.0..22.0);
            let bh = rng.random_range(ph * 0.28..ph * 0.68);
            let sign = if rng.random_range(0.0..1.0) < 0.42 {
                Some(match rng.random_range(0..6) {
                    0 => SignKind::Open,
                    1 => SignKind::Ramen,
                    2 => SignKind::Hotel,
                    3 => SignKind::Arcade,
                    4 => SignKind::Cafe,
                    _ => SignKind::Tech,
                })
            } else {
                None
            };

            self.mid_buildings.push(Building {
                x,
                w: bw,
                h: bh,
                depth: 0.70,
                color: random_building_color(&mut rng, false),
                window_color: random_window_color(&mut rng),
                sign,
                seed: rng.random_range(0.0..9999.0),
            });
            x += bw + rng.random_range(1.0..4.0);
        }

        let lanes = 3usize;
        for i in 0..12 {
            let going_right = i % 2 == 0;
            let kind = match rng.random_range(0..5) {
                0 => VehicleKind::Sedan,
                1 => VehicleKind::Taxi,
                2 => VehicleKind::Bus,
                3 => VehicleKind::Van,
                _ => VehicleKind::Sports,
            };

            let length = match kind {
                VehicleKind::Bus => rng.random_range(18.0..28.0),
                VehicleKind::Van => rng.random_range(13.0..18.0),
                VehicleKind::Sports => rng.random_range(9.0..13.0),
                _ => rng.random_range(10.0..16.0),
            };

            let lane = i % lanes;
            let base_speed = match lane {
                0 => rng.random_range(18.0..32.0),
                1 => rng.random_range(12.0..25.0),
                _ => rng.random_range(22.0..38.0),
            };

            self.vehicles.push(Vehicle {
                x: rng.random_range(-w..w * 2.0),
                lane,
                speed: base_speed,
                length,
                color: random_vehicle_color(&mut rng, kind),
                kind,
                going_right,
                seed: rng.random_range(0.0..9999.0),
            });
        }

        for i in 0..10 {
            self.pedestrians.push(Pedestrian {
                x: rng.random_range(-w..w * 2.0),
                y: ph * 0.73 + rng.random_range(-1.0..4.0),
                speed: if i % 2 == 0 {
                    rng.random_range(4.0..9.0)
                } else {
                    -rng.random_range(4.0..9.0)
                },
                color: random_person_color(&mut rng),
                phase: rng.random_range(0.0..std::f64::consts::TAU),
            });
        }

        self.last_dims = Some((width, height));
    }

    fn spawn_particle(&mut self, width: u16, height: u16, kind: ParticleKind, x: f64, y: f64) {
        let mut rng = rand::rng();
        let w = width.max(1) as f64;
        let ph = height.max(1) as f64 * 2.0;

        let (vx, vy, life, color) = match kind {
            ParticleKind::Rain => (
                rng.random_range(-10.0..-4.0),
                rng.random_range(55.0..82.0),
                rng.random_range(0.45..0.85),
                match self.color_provider.mode {
                    ColorMode::Ocean => (105, 180, 215),
                    ColorMode::Matrix => (80, 210, 120),
                    _ => (120, 165, 190),
                },
            ),
            ParticleKind::Exhaust => (
                rng.random_range(-4.0..1.0),
                rng.random_range(-7.0..-1.0),
                rng.random_range(0.7..1.6),
                (92, 88, 90),
            ),
            ParticleKind::Spark => (
                rng.random_range(-3.0..3.0),
                rng.random_range(-8.0..-2.0),
                rng.random_range(0.25..0.55),
                (255, 190, 80),
            ),
            ParticleKind::Reflection => (
                rng.random_range(-1.0..1.0),
                rng.random_range(4.0..11.0),
                rng.random_range(0.25..0.75),
                (130, 190, 210),
            ),
        };

        self.particles.push(Particle {
            x: x.clamp(-20.0, w + 20.0),
            y: y.clamp(-20.0, ph + 20.0),
            vx,
            vy,
            life,
            max_life: life,
            color,
            kind,
        });
    }
}

impl Mode for StreetParallaxMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        if self.last_dims != Some((width, height)) || self.mid_buildings.is_empty() {
            self.reset_scene(width, height);
        }

        let dt = (dt * self.speed).min(0.05);
        self.time += dt;
        self.camera += dt * 14.0;

        let mut rng = rand::rng();
        let w = width.max(1) as f64;
        let ph = height.max(1) as f64 * 2.0;

        if self.weather_rain {
            for _ in 0..12 {
                if rng.random_range(0.0..1.0) < 0.65 {
                    self.spawn_particle(
                        width,
                        height,
                        ParticleKind::Rain,
                        rng.random_range(0.0..w),
                        rng.random_range(-8.0..2.0),
                    );
                }
            }
        }

        let road_top = ph * 0.75;

        // Defer particle spawns until after the vehicle mutable borrow ends.
        // Calling self.spawn_particle(...) while holding &mut self.vehicles[i]
        // creates a second mutable borrow of self, which Rust correctly rejects.
        let mut deferred_spawns: Vec<(ParticleKind, f64, f64)> = Vec::new();

        for i in 0..self.vehicles.len() {
            let v = &mut self.vehicles[i];
            let dir = if v.going_right { 1.0 } else { -1.0 };
            let lane_speed_factor = 1.0 + v.lane as f64 * 0.12;
            v.x += dir * v.speed * lane_speed_factor * dt;

            if v.going_right && v.x > w + 80.0 {
                v.x = -v.length - rng.random_range(20.0..120.0);
                v.color = random_vehicle_color(&mut rng, v.kind);
            } else if !v.going_right && v.x < -v.length - 80.0 {
                v.x = w + rng.random_range(20.0..120.0);
                v.color = random_vehicle_color(&mut rng, v.kind);
            }

            let car_y = lane_y(ph, v.lane);

            if rng.random_range(0.0..1.0) < 0.035 {
                let exhaust_x = if v.going_right {
                    v.x - 2.0
                } else {
                    v.x + v.length + 2.0
                };

                deferred_spawns.push((ParticleKind::Exhaust, exhaust_x, car_y - 2.0));
            }

            if self.weather_rain && rng.random_range(0.0..1.0) < 0.04 {
                deferred_spawns.push((
                    ParticleKind::Reflection,
                    v.x + rng.random_range(0.0..v.length),
                    road_top + rng.random_range(4.0..18.0),
                ));
            }
        }

        for (kind, x, y) in deferred_spawns {
            self.spawn_particle(width, height, kind, x, y);
        }

        for p in &mut self.pedestrians {
            p.x += p.speed * dt;
            p.phase += dt * 7.0;

            if p.speed > 0.0 && p.x > w + 20.0 {
                p.x = -20.0;
                p.color = random_person_color(&mut rng);
            } else if p.speed < 0.0 && p.x < -20.0 {
                p.x = w + 20.0;
                p.color = random_person_color(&mut rng);
            }
        }

        if rng.random_range(0.0..1.0) < 0.05 {
            self.spawn_particle(
                width,
                height,
                ParticleKind::Spark,
                rng.random_range(0.0..w),
                rng.random_range(ph * 0.30..ph * 0.66),
            );
        }

        for p in &mut self.particles {
            p.x += p.vx * dt;
            p.y += p.vy * dt;

            match p.kind {
                ParticleKind::Rain => {}
                ParticleKind::Exhaust => {
                    p.vy -= 1.5 * dt;
                    p.vx += (self.time * 2.0 + p.y * 0.07).sin() * dt * 1.5;
                }
                ParticleKind::Spark => {
                    p.vy += 12.0 * dt;
                    p.vx *= 0.96;
                }
                ParticleKind::Reflection => {
                    p.vy += 4.0 * dt;
                    p.vx *= 0.97;
                }
            }

            p.life -= dt;
        }

        self.particles.retain(|p| p.life > 0.0 && p.x > -30.0 && p.x < w + 30.0 && p.y > -30.0 && p.y < ph + 40.0);

        if self.particles.len() > 900 {
            let drop = self.particles.len() - 900;
            self.particles.drain(..drop);
        }
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w = width.max(1) as usize;
        let h = height.max(1) as usize;
        let ph = h * 2;

        let mut pix: Pix = vec![vec![(0, 0, 0); w]; ph];

        paint_sky(&mut pix, w, ph, self.color_provider.mode, self.time, self.weather_rain);
        paint_far_glow(&mut pix, w, ph, self.color_provider.mode, self.time);

        for b in &self.far_buildings {
            paint_building_layer(&mut pix, w, ph, b, self.camera, self.color_provider.mode, self.time);
        }

        paint_elevated_tracks(&mut pix, w, ph, self.time, self.camera, self.color_provider.mode);

        for b in &self.mid_buildings {
            paint_building_layer(&mut pix, w, ph, b, self.camera, self.color_provider.mode, self.time);
        }

        paint_sidewalk(&mut pix, w, ph, self.color_provider.mode, self.time, self.weather_rain);
        paint_road(&mut pix, w, ph, self.color_provider.mode, self.time, self.weather_rain);

        for p in &self.pedestrians {
            paint_pedestrian(&mut pix, w, ph, p, self.time);
        }

        for v in &self.vehicles {
            paint_vehicle(&mut pix, w, ph, v, self.time, self.weather_rain);
        }

        paint_foreground_poles(&mut pix, w, ph, self.camera, self.time, self.color_provider.mode);

        for p in &self.particles {
            paint_particle(&mut pix, w, ph, p);
        }

        paint_vignette(&mut pix, w, ph);

        half_blocks(&pix, w, h, &self.color_provider, t_abs)
    }
}

// ── Scene layout ──────────────────────────────────────────────────────────────

fn lane_y(ph: f64, lane: usize) -> f64 {
    match lane {
        0 => ph * 0.82,
        1 => ph * 0.89,
        _ => ph * 0.95,
    }
}

fn random_building_color(rng: &mut impl RngExt, far: bool) -> Rgb {
    if far {
        match rng.random_range(0..4) {
            0 => (24, 30, 48),
            1 => (28, 32, 50),
            2 => (22, 36, 54),
            _ => (32, 28, 45),
        }
    } else {
        match rng.random_range(0..6) {
            0 => (38, 42, 55),
            1 => (46, 38, 54),
            2 => (34, 46, 58),
            3 => (50, 43, 38),
            4 => (30, 34, 48),
            _ => (45, 45, 50),
        }
    }
}

fn random_window_color(rng: &mut impl RngExt) -> Rgb {
    match rng.random_range(0..5) {
        0 => (255, 205, 110),
        1 => (110, 210, 255),
        2 => (255, 120, 190),
        3 => (140, 255, 160),
        _ => (235, 235, 210),
    }
}

fn random_vehicle_color(rng: &mut impl RngExt, kind: VehicleKind) -> Rgb {
    match kind {
        VehicleKind::Taxi => (235, 185, 45),
        VehicleKind::Bus => match rng.random_range(0..3) {
            0 => (190, 45, 55),
            1 => (45, 95, 190),
            _ => (55, 155, 95),
        },
        VehicleKind::Sports => match rng.random_range(0..4) {
            0 => (220, 55, 65),
            1 => (70, 190, 235),
            2 => (245, 245, 245),
            _ => (180, 80, 220),
        },
        _ => match rng.random_range(0..6) {
            0 => (200, 200, 210),
            1 => (60, 90, 140),
            2 => (160, 45, 55),
            3 => (42, 42, 48),
            4 => (80, 160, 120),
            _ => (210, 100, 55),
        },
    }
}

fn random_person_color(rng: &mut impl RngExt) -> Rgb {
    match rng.random_range(0..7) {
        0 => (220, 80, 80),
        1 => (80, 150, 230),
        2 => (245, 200, 85),
        3 => (90, 210, 130),
        4 => (190, 100, 220),
        5 => (230, 230, 230),
        _ => (75, 75, 80),
    }
}

// ── Painting ─────────────────────────────────────────────────────────────────

fn paint_sky(pix: &mut Pix, w: usize, ph: usize, mode: ColorMode, time: f64, rain: bool) {
    let (top, bottom) = match mode {
        ColorMode::Ocean => ((5, 22, 42), (18, 74, 105)),
        ColorMode::Sunset => ((44, 16, 38), (145, 74, 48)),
        ColorMode::Matrix => ((0, 12, 8), (8, 40, 22)),
        ColorMode::Rainbow => ((12, 16, 42), (65, 35, 85)),
    };

    for y in 0..ph {
        let yf = y as f64 / ph.max(1) as f64;
        let mut col = lerp(top, bottom, yf.powf(0.78));

        let haze = ((time * 0.12 + yf * 7.0).sin() + 1.0) * 0.5;
        col = blend(col, (255, 110, 180), haze * 0.025);

        if rain {
            col = blend(col, (35, 45, 58), 0.22);
        }

        for x in 0..w {
            pix[y][x] = col;
        }
    }

    // Moon / neon sun.
    let x = w as f64 * 0.76;
    let y = ph as f64 * 0.13;
    let col = match mode {
        ColorMode::Sunset => (255, 190, 110),
        ColorMode::Matrix => (90, 255, 120),
        ColorMode::Ocean => (155, 210, 245),
        ColorMode::Rainbow => (255, 145, 210),
    };

    paint_soft_circle(pix, w, ph, x, y, 7.0, col, 0.62);
    paint_soft_circle(pix, w, ph, x, y, 24.0, col, 0.055);
}

fn paint_far_glow(pix: &mut Pix, w: usize, ph: usize, mode: ColorMode, time: f64) {
    let horizon = ph as f64 * 0.54;
    let col = match mode {
        ColorMode::Ocean => (50, 160, 210),
        ColorMode::Sunset => (255, 120, 70),
        ColorMode::Matrix => (40, 230, 95),
        ColorMode::Rainbow => (210, 90, 255),
    };

    for y in 0..ph {
        let d = ((y as f64 - horizon).abs() / (ph as f64 * 0.28)).clamp(0.0, 1.0);
        let alpha = (1.0 - d).powf(2.0) * 0.10;
        let pulse = ((time * 0.6).sin() + 1.0) * 0.5;

        for x in 0..w {
            pix[y][x] = blend(pix[y][x], col, alpha * (0.70 + pulse * 0.30));
        }
    }
}

fn paint_building_layer(pix: &mut Pix, w: usize, ph: usize, b: &Building, camera: f64, mode: ColorMode, time: f64) {
    let ground = ph as f64 * if b.depth < 0.5 { 0.70 } else { 0.76 };
    let layer_width = w as f64 * 3.0;
    let mut x = b.x - camera * b.depth;
    x = ((x + layer_width) % layer_width) - w as f64 * 0.45;

    let left = x.round() as i32;
    let right = (x + b.w).round() as i32;
    let top = (ground - b.h).round() as i32;
    let bottom = ground.round() as i32;

    if right < 0 || left >= w as i32 {
        return;
    }

    for py in top..=bottom {
        for px in left..=right {
            if !in_bounds(w, ph, px, py) {
                continue;
            }

            let edge = px == left || px == right;
            let mut col = b.color;

            if b.depth < 0.5 {
                col = blend(col, (5, 10, 18), 0.38);
            }

            if edge {
                col = darken(col, 0.22);
            }

            pix[py as usize][px as usize] = blend(pix[py as usize][px as usize], col, 0.94);
        }
    }

    // Windows.
    let win_step_x = if b.depth < 0.5 { 5 } else { 4 };
    let win_step_y = if b.depth < 0.5 { 6 } else { 5 };

    for py in (top + 4..bottom - 2).step_by(win_step_y) {
        for px in (left + 2..right - 1).step_by(win_step_x) {
            if !in_bounds(w, ph, px, py) {
                continue;
            }

            let flicker = hash01(b.seed + px as f64 * 3.1 + py as f64 * 8.7 + (time * 0.25).floor());
            let lit = flicker > if b.depth < 0.5 { 0.48 } else { 0.38 };

            if lit {
                let mut wc = b.window_color;
                wc = match mode {
                    ColorMode::Matrix => blend(wc, (20, 255, 80), 0.45),
                    ColorMode::Ocean => blend(wc, (90, 205, 255), 0.25),
                    ColorMode::Sunset => blend(wc, (255, 150, 75), 0.22),
                    ColorMode::Rainbow => wc,
                };

                let alpha = if b.depth < 0.5 { 0.40 } else { 0.72 };
                paint_rect(pix, w, ph, px as f64, py as f64, 1.7, 1.7, wc, alpha);
            }
        }
    }

    if b.depth >= 0.5 {
        if let Some(sign) = b.sign {
            paint_sign(pix, w, ph, left, top, right - left, sign, mode, time + b.seed);
        }
    }
}

fn paint_sign(pix: &mut Pix, w: usize, ph: usize, left: i32, top: i32, bw: i32, sign: SignKind, mode: ColorMode, time: f64) {
    let sx = left + bw / 2 - 4;
    let sy = top + 7;

    if bw < 9 || sy < 2 {
        return;
    }

    let base = match sign {
        SignKind::Open => (255, 60, 120),
        SignKind::Ramen => (255, 120, 60),
        SignKind::Hotel => (100, 180, 255),
        SignKind::Arcade => (180, 90, 255),
        SignKind::Cafe => (245, 200, 120),
        SignKind::Tech => (90, 245, 190),
    };

    let col = match mode {
        ColorMode::Matrix => blend(base, (60, 255, 100), 0.35),
        ColorMode::Ocean => blend(base, (80, 180, 255), 0.25),
        ColorMode::Sunset => blend(base, (255, 110, 55), 0.18),
        ColorMode::Rainbow => base,
    };

    let pulse = ((time * 4.0).sin() + 1.0) * 0.5;
    paint_rect(pix, w, ph, sx as f64, sy as f64, 8.0, 3.0, (10, 8, 14), 0.72);
    paint_rect(pix, w, ph, sx as f64 + 1.0, sy as f64 + 1.0, 6.0, 1.0, col, 0.55 + pulse * 0.32);
    paint_soft_circle(pix, w, ph, sx as f64 + 4.0, sy as f64 + 1.5, 7.0, col, 0.045 + pulse * 0.03);
}

fn paint_elevated_tracks(pix: &mut Pix, w: usize, ph: usize, time: f64, camera: f64, mode: ColorMode) {
    let y = ph as f64 * 0.56;
    let rail_col = match mode {
        ColorMode::Matrix => (20, 90, 50),
        _ => (44, 44, 52),
    };

    for x in 0..w {
        paint_rect(pix, w, ph, x as f64, y, 1.0, 2.0, rail_col, 0.78);
    }

    for i in 0..((w / 8) + 8) {
        let x = ((i as f64 * 8.0 - camera * 0.5) % (w as f64 + 16.0)) - 8.0;
        draw_soft_line(pix, w, ph, x, y, x + 7.0, y + 5.0, 0.7, darken(rail_col, 0.25), 0.72);
    }

    // Occasional train blur.
    let train_x = ((time * 25.0 - camera * 0.1) % (w as f64 + 90.0)) - 90.0;
    let train_col = match mode {
        ColorMode::Matrix => (30, 155, 75),
        ColorMode::Ocean => (45, 115, 155),
        ColorMode::Sunset => (150, 65, 45),
        ColorMode::Rainbow => (100, 60, 145),
    };

    if train_x > -85.0 && train_x < w as f64 + 10.0 {
        paint_rect(pix, w, ph, train_x, y - 8.0, 62.0, 7.0, train_col, 0.55);
        for k in 0..8 {
            paint_rect(pix, w, ph, train_x + 5.0 + k as f64 * 7.0, y - 6.0, 2.0, 2.0, (230, 230, 180), 0.55);
        }
    }
}

fn paint_sidewalk(pix: &mut Pix, w: usize, ph: usize, mode: ColorMode, time: f64, rain: bool) {
    let top = (ph as f64 * 0.70) as usize;
    let bottom = (ph as f64 * 0.78) as usize;

    let base = match mode {
        ColorMode::Matrix => (18, 28, 22),
        ColorMode::Ocean => (42, 52, 60),
        ColorMode::Sunset => (72, 48, 42),
        ColorMode::Rainbow => (50, 42, 62),
    };

    for y in top..bottom.min(ph) {
        for x in 0..w {
            let tile = (x / 8 + y / 4) % 2 == 0;
            let mut col = if tile { base } else { darken(base, 0.10) };
            if rain {
                let glint = ((x as f64 * 0.18 + time * 2.0).sin() + 1.0) * 0.5;
                col = blend(col, (100, 125, 145), glint * 0.045);
            }
            pix[y][x] = col;
        }
    }

    // curb
    paint_rect(pix, w, ph, 0.0, bottom as f64 - 1.0, w as f64, 2.0, (105, 95, 85), 0.58);
}

fn paint_road(pix: &mut Pix, w: usize, ph: usize, mode: ColorMode, time: f64, rain: bool) {
    let top = (ph as f64 * 0.78) as usize;
    let base = match mode {
        ColorMode::Matrix => (5, 16, 12),
        ColorMode::Ocean => (18, 28, 38),
        ColorMode::Sunset => (35, 24, 27),
        ColorMode::Rainbow => (24, 22, 35),
    };

    for y in top..ph {
        let depth = (y - top) as f64 / (ph - top).max(1) as f64;
        for x in 0..w {
            let noise = ((x as f64 * 0.9 + y as f64 * 0.4 + time * 0.4).sin() + 1.0) * 0.5;
            let mut col = blend(base, darken(base, 0.35), depth * 0.45);
            col = blend(col, (80, 80, 82), noise * 0.025);

            if rain {
                let refl = ((x as f64 * 0.12 + y as f64 * 0.08 + time * 1.6).sin() + 1.0) * 0.5;
                col = blend(col, (80, 115, 135), refl * 0.08);
            }

            pix[y][x] = col;
        }
    }

    // lane markers
    for lane in 0..3 {
        let y = lane_y(ph as f64, lane) as i32 + 4;
        for x in (0..w as i32).step_by(16) {
            paint_rect(pix, w, ph, x as f64, y as f64, 8.0, 1.0, (205, 190, 135), 0.55);
        }
    }
}

fn paint_vehicle(pix: &mut Pix, w: usize, ph: usize, v: &Vehicle, time: f64, rain: bool) {
    let y = lane_y(ph as f64, v.lane);
    let x = v.x;
    let len = v.length;
    let height = match v.kind {
        VehicleKind::Bus => 8.0,
        VehicleKind::Van => 6.5,
        VehicleKind::Sports => 4.2,
        _ => 5.2,
    };

    let body_y = y - height;
    let col = v.color;
    let shadow_col = (5, 5, 7);

    paint_soft_circle(pix, w, ph, x + len * 0.5, y + 1.6, len * 0.55, shadow_col, 0.12);
    paint_rect(pix, w, ph, x, body_y, len, height, col, 0.88);

    // cabin / roof
    match v.kind {
        VehicleKind::Sports => {
            paint_rect(pix, w, ph, x + len * 0.25, body_y - 2.0, len * 0.42, 2.5, brighten(col, 26), 0.78);
        }
        VehicleKind::Bus => {
            paint_rect(pix, w, ph, x + 2.0, body_y + 1.0, len - 4.0, 2.0, (160, 205, 220), 0.55);
            for k in 0..5 {
                paint_rect(pix, w, ph, x + 3.0 + k as f64 * 4.0, body_y + 1.0, 1.6, 2.0, (225, 235, 210), 0.45);
            }
        }
        _ => {
            paint_rect(pix, w, ph, x + len * 0.28, body_y - 2.0, len * 0.45, 3.0, brighten(col, 28), 0.72);
        }
    }

    // wheels
    let wheel_y = y + 0.4;
    paint_soft_circle(pix, w, ph, x + len * 0.22, wheel_y, 2.0, (4, 4, 5), 0.9);
    paint_soft_circle(pix, w, ph, x + len * 0.78, wheel_y, 2.0, (4, 4, 5), 0.9);
    paint_soft_circle(pix, w, ph, x + len * 0.22, wheel_y, 0.9, (115, 115, 120), 0.6);
    paint_soft_circle(pix, w, ph, x + len * 0.78, wheel_y, 0.9, (115, 115, 120), 0.6);

    // lights
    let pulse = ((time * 4.0 + v.seed).sin() + 1.0) * 0.5;
    if v.going_right {
        paint_soft_circle(pix, w, ph, x + len + 1.0, body_y + height * 0.62, 2.2, (255, 230, 155), 0.40 + pulse * 0.10);
        paint_soft_circle(pix, w, ph, x - 0.8, body_y + height * 0.65, 1.4, (255, 45, 45), 0.44);
    } else {
        paint_soft_circle(pix, w, ph, x - 1.0, body_y + height * 0.62, 2.2, (255, 230, 155), 0.40 + pulse * 0.10);
        paint_soft_circle(pix, w, ph, x + len + 0.8, body_y + height * 0.65, 1.4, (255, 45, 45), 0.44);
    }

    if rain {
        let refl_y = y + 6.0;
        let refl_col = blend(col, (120, 160, 180), 0.45);
        paint_rect(pix, w, ph, x + 1.0, refl_y, len - 2.0, 2.0, refl_col, 0.12);
    }
}

fn paint_pedestrian(pix: &mut Pix, w: usize, ph: usize, p: &Pedestrian, _time: f64) {
    let x = p.x.round() as i32;
    let y = p.y.round() as i32;
    let leg = p.phase.sin().round() as i32;

    paint_soft_circle(pix, w, ph, x as f64, (y - 5) as f64, 1.2, (210, 170, 130), 0.85);
    paint_rect(pix, w, ph, x as f64 - 1.0, y as f64 - 4.0, 2.0, 4.0, p.color, 0.82);

    set_blend(pix, w, ph, x - 1, y + leg, (25, 25, 28), 0.8);
    set_blend(pix, w, ph, x + 1, y - leg, (25, 25, 28), 0.8);
}

fn paint_foreground_poles(pix: &mut Pix, w: usize, ph: usize, camera: f64, time: f64, mode: ColorMode) {
    let spacing = 26.0;
    let base_y = ph as f64 * 0.72;
    let col = (34, 32, 36);
    let lamp_col = match mode {
        ColorMode::Matrix => (90, 255, 120),
        ColorMode::Ocean => (150, 220, 255),
        ColorMode::Sunset => (255, 170, 90),
        ColorMode::Rainbow => (255, 120, 210),
    };

    for i in 0..((w as f64 / spacing) as i32 + 4) {
        let x = ((i as f64 * spacing - camera * 1.4) % (w as f64 + spacing)) - spacing;
        let x = x.round() as i32;

        for y in (base_y as i32 - 20)..base_y as i32 {
            set_blend(pix, w, ph, x, y, col, 0.82);
        }

        draw_soft_line(pix, w, ph, x as f64, base_y - 20.0, x as f64 + 7.0, base_y - 23.0, 0.8, col, 0.9);
        let pulse = ((time * 2.0 + i as f64).sin() + 1.0) * 0.5;
        paint_soft_circle(pix, w, ph, x as f64 + 8.0, base_y - 23.0, 3.0, lamp_col, 0.32 + pulse * 0.08);
        paint_soft_circle(pix, w, ph, x as f64 + 8.0, base_y - 23.0, 10.0, lamp_col, 0.030 + pulse * 0.025);
    }
}

fn paint_particle(pix: &mut Pix, w: usize, ph: usize, p: &Particle) {
    let fade = (p.life / p.max_life).clamp(0.0, 1.0);

    match p.kind {
        ParticleKind::Rain => {
            for i in 0..5 {
                set_blend(
                    pix,
                    w,
                    ph,
                    p.x.round() as i32 - i,
                    p.y.round() as i32 + i,
                    p.color,
                    fade * (0.42 - i as f64 * 0.055),
                );
            }
        }
        ParticleKind::Exhaust => {
            paint_soft_circle(pix, w, ph, p.x, p.y, 3.2 * (1.0 - fade + 0.2), p.color, fade * 0.20);
        }
        ParticleKind::Spark => {
            paint_soft_circle(pix, w, ph, p.x, p.y, 1.2, p.color, fade * 0.55);
        }
        ParticleKind::Reflection => {
            paint_soft_circle(pix, w, ph, p.x, p.y, 1.6, p.color, fade * 0.20);
        }
    }
}

fn paint_vignette(pix: &mut Pix, w: usize, ph: usize) {
    for y in 0..ph {
        for x in 0..w {
            let nx = (x as f64 / w.max(1) as f64 - 0.5).abs() * 2.0;
            let ny = (y as f64 / ph.max(1) as f64 - 0.5).abs() * 2.0;
            let d = ((nx * nx + ny * ny) * 0.5).clamp(0.0, 1.0);
            pix[y][x] = darken(pix[y][x], d.powf(2.2) * 0.25);
        }
    }
}

// ── Primitive drawing ─────────────────────────────────────────────────────────

fn in_bounds(w: usize, ph: usize, x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < w as i32 && y < ph as i32
}


fn set_blend(pix: &mut Pix, w: usize, ph: usize, x: i32, y: i32, col: Rgb, alpha: f64) {
    if in_bounds(w, ph, x, y) {
        let current = pix[y as usize][x as usize];
        pix[y as usize][x as usize] = blend(current, col, alpha);
    }
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
                let a = (1.0 - d / r.max(1.0)).powf(1.45) * power;
                pix[py as usize][px as usize] =
                    blend(pix[py as usize][px as usize], col, a);
            }
        }
    }
}

fn draw_soft_line(
    pix: &mut Pix,
    w: usize,
    ph: usize,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    radius: f64,
    col: Rgb,
    power: f64,
) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let steps = (dx.abs().max(dy.abs()) as usize + 1).max(1);

    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let x = x1 + dx * t;
        let y = y1 + dy * t;
        paint_soft_circle(pix, w, ph, x, y, radius, col, power);
    }
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

fn street_theme_tint(color_provider: &ColorProvider, base: Rgb, t: f64, x: i32, y: i32) -> Rgb {
    // Unlike some modes, this one does use the selected theme as art direction,
    // but it is not a full rainbow overlay. It grades the scene like a filter.
    match color_provider.mode {
        ColorMode::Rainbow => {
            let neon = ((t * 0.7 + (x + y) as f64 * 0.025).sin() + 1.0) * 0.5;
            blend(base, (210, 80, 255), 0.045 + neon * 0.045)
        }
        ColorMode::Ocean => {
            blend(base, (50, 135, 180), 0.16)
        }
        ColorMode::Sunset => {
            blend(base, (255, 120, 70), 0.17)
        }
        ColorMode::Matrix => {
            blend(base, (30, 210, 85), 0.22)
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

            let fg = street_theme_tint(color_provider, base_fg, t_abs, x as i32, upper as i32);
            let bg = street_theme_tint(color_provider, base_bg, t_abs, x as i32, lower as i32);

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
