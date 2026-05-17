// ===== src/modes/bounce.rs =====
//
// BounceMode v3
//
// A kinetic physics wallpaper mode designed to NEVER go dormant.
//
// Main change from v2:
//   Instead of trying to wake balls only after they fall asleep, this version
//   actively manages energy during every wall/collision event.
//
// Physics style:
//   - collisions ADD a little energy instead of always removing it
//   - velocities are clamped to a safe range
//   - low-energy balls get nudged before the whole system dies
//   - higher speed setting makes gravity stronger, but with sqrt scaling so it
//     does not become uncontrollable
//
// Visual style:
//   - half-block pixel renderer
//   - glowing balls
//   - motion trails
//   - impact rings
//   - sparks
//   - subtle arena grid/floor shimmer

use crate::ansi::{bg_rgb, rgb, RESET};
use crate::color::ColorProvider;
use crate::mode_base::Mode;
use rand::RngExt;
use std::f64::consts::TAU;

type Rgb = (u8, u8, u8);
type Pix = Vec<Vec<Rgb>>;

#[derive(Clone, Debug)]
struct Ball {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    radius: f64,
    mass: f64,
    seed: i32,
    hue_seed: f64,
    spin: f64,
    trail: Vec<(f64, f64, f64)>, // x, y, age
    idle_time: f64,
}

#[derive(Clone, Copy, Debug)]
struct Spark {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    life: f64,
    max_life: f64,
    color: Rgb,
}

#[derive(Clone, Copy, Debug)]
struct ImpactRing {
    x: f64,
    y: f64,
    radius: f64,
    speed: f64,
    life: f64,
    max_life: f64,
    color: Rgb,
}

pub struct BounceMode {
    speed_factor: f64,
    color_provider: ColorProvider,
    balls: Vec<Ball>,
    sparks: Vec<Spark>,
    rings: Vec<ImpactRing>,
    last_dims: Option<(u16, u16)>,

    // Simulation tuning.
    base_gravity: f64,
    wall_energy_gain: f64,
    collision_energy_gain: f64,
    friction: f64,
    min_speed: f64,
    max_speed: f64,
    target_ball_count: usize,
    time: f64,
}

impl BounceMode {
    pub fn new(speed_factor: f64, color_provider: ColorProvider) -> Self {
        // sqrt/log-like scaling:
        // Higher speed setting makes gravity stronger, but not linearly insane.
        let speed_scale = speed_factor.max(0.25).sqrt();

        Self {
            speed_factor,
            color_provider,
            balls: Vec::new(),
            sparks: Vec::new(),
            rings: Vec::new(),
            last_dims: None,

            base_gravity: 33.0 * speed_scale,
            wall_energy_gain: 1.045,
            collision_energy_gain: 1.035,
            friction: 0.998,
            min_speed: 14.0 * speed_scale,
            max_speed: 72.0 * speed_scale,
            target_ball_count: 7,
            time: 0.0,
        }
    }

    fn init_balls(&mut self, width: u16, height: u16) {
        self.balls.clear();
        self.sparks.clear();
        self.rings.clear();

        let mut rng = rand::rng();

        let w = width.max(10) as f64;
        let ph = height.max(10) as f64 * 2.0;

        let min_dim = w.min(ph);
        let base_radius = (min_dim / 18.0).clamp(3.0, 7.0);

        let cx = w / 2.0;
        let cy = ph * 0.25;
        let ring_r = min_dim * 0.18;

        for i in 0..self.target_ball_count {
            let angle = TAU * (i as f64 / self.target_ball_count as f64)
                + rng.random_range(-0.35..0.35);

            let radius = rng.random_range(base_radius * 0.72..base_radius * 1.18);
            let mass = radius * radius;

            let mut x = cx + ring_r * angle.cos();
            let mut y = cy + ring_r * angle.sin();

            x = x.clamp(radius + 2.0, w - radius - 2.0);
            y = y.clamp(radius + 2.0, ph - radius - 4.0);

            let initial_speed = rng.random_range(self.min_speed * 1.1..self.min_speed * 2.2);
            let dir: f64 = rng.random_range(0.0..TAU);

            self.balls.push(Ball {
                x,
                y,
                vx: dir.cos() * initial_speed,
                vy: dir.sin() * initial_speed - rng.random_range(4.0..12.0),
                radius,
                mass,
                seed: rng.random_range(0..10000),
                hue_seed: rng.random_range(0.0..TAU),
                spin: rng.random_range(-2.0..2.0),
                trail: Vec::with_capacity(22),
                idle_time: 0.0,
            });
        }
    }

    fn ball_color(&self, ball: &Ball) -> Rgb {
        let r = ((self.time * 1.4 + ball.hue_seed).sin() * 65.0 + 170.0) as u8;
        let g = ((self.time * 1.2 + ball.hue_seed + 2.1).sin() * 65.0 + 170.0) as u8;
        let b = ((self.time * 1.1 + ball.hue_seed + 4.2).sin() * 65.0 + 170.0) as u8;
        (r, g, b)
    }

    fn spawn_impact(&mut self, x: f64, y: f64, power: f64, base: Rgb) {
        let mut rng = rand::rng();
        let count = (4.0 + power * 0.10).clamp(4.0, 18.0) as usize;

        self.rings.push(ImpactRing {
            x,
            y,
            radius: 1.0,
            speed: (10.0 + power * 0.10).clamp(10.0, 30.0),
            life: 0.38,
            max_life: 0.38,
            color: base,
        });

        for _ in 0..count {
            let angle: f64 = rng.random_range(0.0..TAU);
            let speed = rng.random_range(4.0..17.0) * (0.55 + power * 0.018).clamp(0.65, 2.0);

            self.sparks.push(Spark {
                x,
                y,
                vx: angle.cos() * speed,
                vy: angle.sin() * speed - rng.random_range(0.0..7.0),
                life: rng.random_range(0.16..0.52),
                max_life: 0.52,
                color: base,
            });
        }
    }

    fn speed_of(vx: f64, vy: f64) -> f64 {
        (vx * vx + vy * vy).sqrt()
    }

    fn clamp_ball_velocity(min_speed: f64, max_speed: f64, b: &mut Ball, rng: &mut impl RngExt) {
        let speed = Self::speed_of(b.vx, b.vy);

        if speed < 0.001 {
            let angle: f64 = rng.random_range(0.0..TAU);
            b.vx = angle.cos() * min_speed;
            b.vy = angle.sin() * min_speed;
            return;
        }

        // If the ball is too slow, scale the whole vector upward.
        if speed < min_speed {
            let scale = min_speed / speed;
            b.vx *= scale;
            b.vy *= scale;
        }

        // If the ball is too fast, clamp it so the mode does not explode.
        let speed = Self::speed_of(b.vx, b.vy);
        if speed > max_speed {
            let scale = max_speed / speed;
            b.vx *= scale;
            b.vy *= scale;
        }

        // Avoid perfectly vertical motion, which is visually boring.
        if b.vx.abs() < min_speed * 0.10 {
            let dir = if rng.random_range(0..2) == 0 { -1.0 } else { 1.0 };
            b.vx += dir * rng.random_range(min_speed * 0.12..min_speed * 0.28);
        }
    }

    fn energetic_wall_bounce(
        min_speed: f64,
        max_speed: f64,
        wall_energy_gain: f64,
        friction: f64,
        b: &mut Ball,
        normal_x: f64,
        normal_y: f64,
        rng: &mut impl RngExt,
    ) -> f64 {
        let before = Self::speed_of(b.vx, b.vy);

        // Reflect velocity around the wall normal.
        let dot = b.vx * normal_x + b.vy * normal_y;
        b.vx -= 2.0 * dot * normal_x;
        b.vy -= 2.0 * dot * normal_y;

        // Add controlled energy on every bounce.
        // This is the main "wallpaper physics" trick: it will not settle forever.
        let bounce_gain = wall_energy_gain + rng.random_range(0.00..0.045);
        b.vx *= bounce_gain;
        b.vy *= bounce_gain;

        // Tangential slip.
        // For floor/ceiling, tangent is horizontal. For side walls, tangent is vertical.
        let tx = -normal_y;
        let ty = normal_x;
        let slip = rng.random_range(-2.0..2.0);
        b.vx += tx * slip;
        b.vy += ty * slip;

        // Keep a tiny amount of damping so it does not become infinite chaos.
        b.vx *= friction;
        b.vy *= friction;

        Self::clamp_ball_velocity(min_speed, max_speed, b, rng);

        before
    }

    fn handle_ball_collisions(&mut self) {
        let n = self.balls.len();
        if n < 2 {
            return;
        }

        let mut impacts: Vec<(f64, f64, f64, Rgb)> = Vec::new();
        let mut rng = rand::rng();

        for i in 0..n {
            let (left, right) = self.balls.split_at_mut(i + 1);
            let b1 = &mut left[i];

            for b2 in right.iter_mut() {
                let dx = b2.x - b1.x;
                let dy = b2.y - b1.y;
                let rs = b1.radius + b2.radius;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq <= 0.0001 || dist_sq > rs * rs {
                    continue;
                }

                let dist = dist_sq.sqrt();
                let nx = dx / dist;
                let ny = dy / dist;

                // Separate overlapping balls by mass.
                let overlap = rs - dist;
                let total_mass = b1.mass + b2.mass;
                let b1_push = b2.mass / total_mass;
                let b2_push = b1.mass / total_mass;

                b1.x -= nx * overlap * b1_push;
                b1.y -= ny * overlap * b1_push;
                b2.x += nx * overlap * b2_push;
                b2.y += ny * overlap * b2_push;

                let rvx = b2.vx - b1.vx;
                let rvy = b2.vy - b1.vy;
                let rel_vn = rvx * nx + rvy * ny;

                if rel_vn > 0.0 {
                    continue;
                }

                // Impulse collision with a small energy gain.
                let e = self.collision_energy_gain;
                let impulse = -(1.0 + e) * rel_vn / (1.0 / b1.mass + 1.0 / b2.mass);

                b1.vx -= impulse * nx / b1.mass;
                b1.vy -= impulse * ny / b1.mass;
                b2.vx += impulse * nx / b2.mass;
                b2.vy += impulse * ny / b2.mass;

                // Random microscopic tangent impulse keeps collisions from looking too perfect.
                let tx = -ny;
                let ty = nx;
                let tangent = rng.random_range(-1.6..1.6);
                b1.vx -= tx * tangent / b1.mass.sqrt();
                b1.vy -= ty * tangent / b1.mass.sqrt();
                b2.vx += tx * tangent / b2.mass.sqrt();
                b2.vy += ty * tangent / b2.mass.sqrt();

                b1.spin -= tangent * 0.05;
                b2.spin += tangent * 0.05;

                Self::clamp_ball_velocity(self.min_speed, self.max_speed, b1, &mut rng);
                Self::clamp_ball_velocity(self.min_speed, self.max_speed, b2, &mut rng);

                if impulse.abs() > 18.0 {
                    let ix = b1.x + nx * b1.radius;
                    let iy = b1.y + ny * b1.radius;
                    impacts.push((ix, iy, impulse.abs(), (220, 235, 255)));
                }
            }
        }

        for (x, y, power, color) in impacts {
            self.spawn_impact(x, y, power, color);
        }
    }
}

impl Mode for BounceMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        let dims = (width, height);
        if self.last_dims != Some(dims) || self.balls.is_empty() {
            self.last_dims = Some(dims);
            if width > 8 && height > 8 {
                self.init_balls(width, height);
            } else {
                return;
            }
        }

        // dt is intentionally not multiplied by speed_factor here because gravity and
        // initial tuning already scale with speed. Double-scaling makes fast modes too chaotic.
        let dt = dt.min(0.05);
        self.time += dt * self.speed_factor.max(0.25);

        let w = width.max(1) as f64;
        let ph = height.max(1) as f64 * 2.0;

        let floor = ph - 2.0;
        let ceiling = 1.0;
        let left = 0.5;
        let right = w - 1.5;

        let mut impacts: Vec<(f64, f64, f64, Rgb)> = Vec::new();
        let mut rng = rand::rng();

        let base_gravity = self.base_gravity;
        let min_speed = self.min_speed;
        let max_speed = self.max_speed;
        let wall_energy_gain = self.wall_energy_gain;
        let friction = self.friction;

        for b in &mut self.balls {
            b.trail.insert(0, (b.x, b.y, 0.0));
            if b.trail.len() > 22 {
                b.trail.truncate(22);
            }

            for p in &mut b.trail {
                p.2 += dt;
            }

            // Gravity has a sqrt-scaled base so faster modes drop faster,
            // but not so fast that everything becomes unreadable.
            b.vy += base_gravity * dt;

            b.x += b.vx * dt;
            b.y += b.vy * dt;
            b.spin += (b.vx / b.radius.max(1.0)) * dt * 0.45;

            let r = b.radius;

            // Floor.
            if b.y >= floor - r {
                b.y = floor - r;
                let impact = Self::energetic_wall_bounce(
                    min_speed,
                    max_speed,
                    wall_energy_gain,
                    friction,
                    b,
                    0.0,
                    -1.0,
                    &mut rng,
                );

                if impact > 10.0 {
                    impacts.push((b.x, floor - r * 0.25, impact, (255, 190, 100)));
                }
            }

            // Ceiling.
            if b.y <= ceiling + r {
                b.y = ceiling + r;
                let impact = Self::energetic_wall_bounce(
                    min_speed,
                    max_speed,
                    wall_energy_gain,
                    friction,
                    b,
                    0.0,
                    1.0,
                    &mut rng,
                );

                if impact > 10.0 {
                    impacts.push((b.x, ceiling + r * 0.25, impact, (160, 220, 255)));
                }
            }

            // Left wall.
            if b.x <= left + r {
                b.x = left + r;
                let impact = Self::energetic_wall_bounce(
                    min_speed,
                    max_speed,
                    wall_energy_gain,
                    friction,
                    b,
                    1.0,
                    0.0,
                    &mut rng,
                );

                if impact > 10.0 {
                    impacts.push((left + r * 0.25, b.y, impact, (180, 255, 220)));
                }
            }

            // Right wall.
            if b.x >= right - r {
                b.x = right - r;
                let impact = Self::energetic_wall_bounce(
                    min_speed,
                    max_speed,
                    wall_energy_gain,
                    friction,
                    b,
                    -1.0,
                    0.0,
                    &mut rng,
                );

                if impact > 10.0 {
                    impacts.push((right - r * 0.25, b.y, impact, (255, 140, 220)));
                }
            }

            // Continuous anti-dormancy:
            // If a ball's speed falls below the target for too long, inject a clean impulse.
            let speed = Self::speed_of(b.vx, b.vy);
            if speed < min_speed * 0.82 {
                b.idle_time += dt;
            } else {
                b.idle_time = 0.0;
            }

            if b.idle_time > 0.25 {
                let angle: f64 = rng.random_range(-2.55..-0.58); // upward-ish
                let impulse = rng.random_range(min_speed * 0.85..min_speed * 1.45);
                b.vx += angle.cos() * impulse;
                b.vy += angle.sin() * impulse;
                b.spin += rng.random_range(-1.4..1.4);
                b.idle_time = 0.0;
                Self::clamp_ball_velocity(min_speed, max_speed, b, &mut rng);
                impacts.push((b.x, b.y, impulse, (255, 240, 150)));
            }

            Self::clamp_ball_velocity(min_speed, max_speed, b, &mut rng);
        }

        for (x, y, power, color) in impacts {
            self.spawn_impact(x, y, power, color);
        }

        self.handle_ball_collisions();

        for s in &mut self.sparks {
            s.x += s.vx * dt;
            s.y += s.vy * dt;
            s.vy += self.base_gravity * 0.50 * dt;
            s.life -= dt;
        }

        self.sparks.retain(|s| {
            s.life > 0.0 && s.x >= -4.0 && s.x <= w + 4.0 && s.y >= -4.0 && s.y <= ph + 4.0
        });

        for r in &mut self.rings {
            r.radius += r.speed * dt;
            r.life -= dt;
        }

        self.rings.retain(|r| r.life > 0.0);

        if self.sparks.len() > 260 {
            let drop = self.sparks.len() - 260;
            self.sparks.drain(..drop);
        }
        if self.rings.len() > 35 {
            let drop = self.rings.len() - 35;
            self.rings.drain(..drop);
        }
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w = width.max(1) as usize;
        let h = height.max(1) as usize;
        let ph = h * 2;

        let mut pix: Pix = vec![vec![(0, 0, 0); w]; ph];

        paint_background(&mut pix, w, ph, self.time);
        paint_floor(&mut pix, w, ph, self.time);

        for b in &self.balls {
            let col = self.ball_color(b);
            paint_trail(&mut pix, w, ph, b, col);
        }

        for ring in &self.rings {
            paint_ring(&mut pix, w, ph, ring);
        }

        for b in &self.balls {
            let col = self.ball_color(b);
            paint_ball(&mut pix, w, ph, b, col, self.time);
        }

        for s in &self.sparks {
            paint_spark(&mut pix, w, ph, s);
        }

        half_blocks(&pix, w, h, &self.color_provider, t_abs)
    }
}

// ── Rendering helpers ─────────────────────────────────────────────────────────

fn paint_background(pix: &mut Pix, w: usize, ph: usize, time: f64) {
    for y in 0..ph {
        let yf = y as f64 / ph.max(1) as f64;
        let top = (5, 8, 24);
        let bottom = (18, 12, 34);
        let mut col = lerp(top, bottom, yf.powf(0.85));

        let pulse = ((time * 0.9 + yf * 7.0).sin() + 1.0) * 0.5;
        col = blend(col, (25, 35, 70), pulse * 0.045);

        for x in 0..w {
            let grid = (x % 8 == 0 || y % 8 == 0) && y > ph / 4;
            pix[y][x] = if grid {
                blend(col, (55, 65, 95), 0.12)
            } else {
                col
            };
        }
    }
}

fn paint_floor(pix: &mut Pix, w: usize, ph: usize, time: f64) {
    let floor_y = ph.saturating_sub(3);

    for y in floor_y..ph {
        for x in 0..w {
            let shimmer = ((x as f64 * 0.21 + time * 2.4).sin() + 1.0) * 0.5;
            let col = lerp((28, 25, 42), (55, 45, 75), shimmer);
            pix[y][x] = blend(pix[y][x], col, 0.58);
        }
    }

    if floor_y < ph {
        for x in 0..w {
            pix[floor_y][x] = blend(pix[floor_y][x], (130, 105, 180), 0.35);
        }
    }
}

fn paint_trail(pix: &mut Pix, w: usize, ph: usize, b: &Ball, col: Rgb) {
    for (i, &(x, y, age)) in b.trail.iter().enumerate().skip(1) {
        let fade = (1.0 - i as f64 / b.trail.len().max(1) as f64).powf(1.35);
        let age_fade = (1.0 - age * 1.6).clamp(0.0, 1.0);
        let alpha = fade * age_fade * 0.34;

        if alpha <= 0.01 {
            continue;
        }

        let r = (b.radius * (0.58 + fade * 0.28)).max(1.5);
        paint_soft_circle(pix, w, ph, x, y, r, col, alpha);
    }
}

fn paint_ball(pix: &mut Pix, w: usize, ph: usize, b: &Ball, col: Rgb, time: f64) {
    paint_soft_circle(pix, w, ph, b.x, b.y, b.radius + 2.0, col, 0.12);

    let r = b.radius.ceil() as i32;
    let cx = b.x.round() as i32;
    let cy = b.y.round() as i32;

    for dy in -r..=r {
        for dx in -r..=r {
            let px = cx + dx;
            let py = cy + dy;

            if px < 0 || px >= w as i32 || py < 0 || py >= ph as i32 {
                continue;
            }

            let dist = ((dx * dx + dy * dy) as f64).sqrt();
            if dist > b.radius {
                continue;
            }

            let n = dist / b.radius.max(1.0);
            let light = 1.0 - n;

            let stripe =
                ((dx as f64 * 0.55 + dy as f64 * 0.25 + b.spin * 7.0 + time).sin() + 1.0) * 0.5;
            let mut shade = blend(col, (255, 255, 255), light.powf(2.0) * 0.40);
            shade = blend(shade, (40, 35, 65), n.powf(1.7) * 0.32);
            shade = blend(shade, (255, 255, 255), stripe * 0.10);

            pix[py as usize][px as usize] = blend(pix[py as usize][px as usize], shade, 0.97);
        }
    }

    let hx = cx - (b.radius * 0.35) as i32;
    let hy = cy - (b.radius * 0.45) as i32;
    paint_soft_circle(
        pix,
        w,
        ph,
        hx as f64,
        hy as f64,
        (b.radius * 0.22).max(1.0),
        (255, 255, 255),
        0.38,
    );
}

fn paint_spark(pix: &mut Pix, w: usize, ph: usize, s: &Spark) {
    let fade = (s.life / s.max_life).clamp(0.0, 1.0);
    paint_soft_circle(pix, w, ph, s.x, s.y, 1.4, s.color, fade * 0.75);
}

fn paint_ring(pix: &mut Pix, w: usize, ph: usize, ring: &ImpactRing) {
    let fade = (ring.life / ring.max_life).clamp(0.0, 1.0);
    let r = ring.radius;
    let thickness = 1.2;

    let min_x = (ring.x - r - 2.0).floor() as i32;
    let max_x = (ring.x + r + 2.0).ceil() as i32;
    let min_y = (ring.y - r - 2.0).floor() as i32;
    let max_y = (ring.y + r + 2.0).ceil() as i32;

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            if px < 0 || px >= w as i32 || py < 0 || py >= ph as i32 {
                continue;
            }

            let dx = px as f64 - ring.x;
            let dy = py as f64 - ring.y;
            let d = (dx * dx + dy * dy).sqrt();
            let band = 1.0 - ((d - r).abs() / thickness).clamp(0.0, 1.0);

            if band > 0.0 {
                pix[py as usize][px as usize] =
                    blend(pix[py as usize][px as usize], ring.color, band * fade * 0.34);
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
            if px < 0 || px >= w as i32 || py < 0 || py >= ph as i32 {
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
