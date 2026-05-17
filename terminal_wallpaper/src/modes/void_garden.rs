// ===== src/modes/void_garden.rs =====
//
// VoidGardenMode
//
// A surreal space/biology hybrid mode:
//   black-hole core -> orbiting seed pods -> gravitational dust streams ->
//   blooming energy flowers -> comet particles -> lensing rings
//
// This intentionally mixes symbols with half-block pixel rendering. The base scene
// is smooth RGB pixels, then terminal glyphs are blended in by painting small
// geometric forms into the pixel buffer.
//
// It should feel different from Sky Harbor / Aurora City / Castle Siege:
// less "scene", more cosmic simulation wallpaper.

use crate::ansi::{bg_rgb, rgb, RESET};
use crate::color::ColorProvider;
use crate::mode_base::Mode;
use rand::RngExt;
use std::f64::consts::TAU;

type Rgb = (u8, u8, u8);
type Pix = Vec<Vec<Rgb>>;

#[derive(Clone, Copy)]
struct Pod {
    orbit_r: f64,
    angle: f64,
    angular_speed: f64,
    size: f64,
    phase: f64,
    color: Rgb,
}

#[derive(Clone, Copy)]
struct Dust {
    angle: f64,
    radius: f64,
    speed: f64,
    drift: f64,
    phase: f64,
}

#[derive(Clone, Copy)]
struct Comet {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    life: f64,
    max_life: f64,
    cooldown: f64,
    color: Rgb,
}

pub struct VoidGardenMode {
    speed: f64,
    color_provider: ColorProvider,
    time: f64,
    pods: Vec<Pod>,
    dust: Vec<Dust>,
    comets: Vec<Comet>,
}

impl VoidGardenMode {
    pub fn new(speed: f64, color_provider: ColorProvider) -> Self {
        let mut rng = rand::rng();

        let pods = (0..13)
            .map(|i| {
                let family = i % 4;
                let color = match family {
                    0 => (130, 255, 210),
                    1 => (255, 120, 210),
                    2 => (155, 150, 255),
                    _ => (255, 210, 100),
                };

                Pod {
                    orbit_r: rng.random_range(18.0..66.0),
                    angle: rng.random_range(0.0..TAU),
                    angular_speed: rng.random_range(-0.32..0.32),
                    size: rng.random_range(2.2..5.8),
                    phase: rng.random_range(0.0..TAU),
                    color,
                }
            })
            .collect();

        let dust = (0..520)
            .map(|_| Dust {
                angle: rng.random_range(0.0..TAU),
                radius: rng.random_range(6.0..95.0),
                speed: rng.random_range(0.08..0.75),
                drift: rng.random_range(-0.12..0.18),
                phase: rng.random_range(0.0..TAU),
            })
            .collect();

        let comets = (0..5)
            .map(|_| Comet {
                x: 0.0,
                y: 0.0,
                vx: 0.0,
                vy: 0.0,
                life: 0.0,
                max_life: 1.0,
                cooldown: rng.random_range(2.0..12.0),
                color: (180, 230, 255),
            })
            .collect();

        Self {
            speed,
            color_provider,
            time: 0.0,
            pods,
            dust,
            comets,
        }
    }
}

impl Mode for VoidGardenMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        let dt = dt * self.speed;
        self.time += dt;

        for pod in &mut self.pods {
            pod.angle += pod.angular_speed * dt;
            pod.phase += dt * 1.2;
        }

        for d in &mut self.dust {
            d.angle += d.speed * dt / (d.radius * 0.08).max(1.0);
            d.radius += d.drift * dt;

            if d.radius < 5.0 {
                d.radius = 95.0;
            } else if d.radius > 105.0 {
                d.radius = 8.0;
            }
        }

        let mut rng = rand::rng();
        let w = width.max(1) as f64;
        let ph = height.max(1) as f64 * 2.0;

        for comet in &mut self.comets {
            if comet.life > 0.0 {
                comet.life -= dt;
                comet.x += comet.vx * dt;
                comet.y += comet.vy * dt;
            } else {
                comet.cooldown -= dt;
                if comet.cooldown <= 0.0 {
                    let side = rng.random_range(0..4);
                    match side {
                        0 => {
                            comet.x = rng.random_range(0.0..w);
                            comet.y = -8.0;
                            comet.vx = rng.random_range(-12.0..12.0);
                            comet.vy = rng.random_range(18.0..38.0);
                        }
                        1 => {
                            comet.x = w + 8.0;
                            comet.y = rng.random_range(0.0..ph * 0.65);
                            comet.vx = rng.random_range(-42.0..-22.0);
                            comet.vy = rng.random_range(5.0..22.0);
                        }
                        2 => {
                            comet.x = rng.random_range(0.0..w);
                            comet.y = ph + 8.0;
                            comet.vx = rng.random_range(-10.0..10.0);
                            comet.vy = rng.random_range(-38.0..-20.0);
                        }
                        _ => {
                            comet.x = -8.0;
                            comet.y = rng.random_range(0.0..ph * 0.65);
                            comet.vx = rng.random_range(22.0..42.0);
                            comet.vy = rng.random_range(5.0..22.0);
                        }
                    }

                    comet.max_life = rng.random_range(0.9..1.8);
                    comet.life = comet.max_life;
                    comet.cooldown = rng.random_range(5.0..18.0);
                    comet.color = match rng.random_range(0..4) {
                        0 => (120, 255, 215),
                        1 => (255, 145, 225),
                        2 => (170, 170, 255),
                        _ => (255, 220, 120),
                    };
                }
            }
        }
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w = width.max(1) as usize;
        let h = height.max(1) as usize;
        let ph = h * 2;

        let mut pix: Pix = vec![vec![(0, 0, 0); w]; ph];

        let cx = w as f64 * 0.5;
        let cy = ph as f64 * 0.50;
        let scale = w.min(ph) as f64 / 90.0;

        paint_space(&mut pix, w, ph, self.time);
        paint_lensing_field(&mut pix, w, ph, cx, cy, self.time);

        for d in &self.dust {
            paint_dust(&mut pix, w, ph, d, cx, cy, scale, self.time);
        }

        for comet in &self.comets {
            paint_comet(&mut pix, w, ph, comet);
        }

        for pod in &self.pods {
            paint_pod(&mut pix, w, ph, pod, cx, cy, scale, self.time);
        }

        paint_core(&mut pix, w, ph, cx, cy, scale, self.time);

        half_blocks(&pix, w, h, &self.color_provider, t_abs)
    }
}

fn paint_space(pix: &mut Pix, w: usize, ph: usize, time: f64) {
    for y in 0..ph {
        let yf = y as f64 / ph.max(1) as f64;
        let top = (2, 3, 12);
        let bottom = (12, 4, 22);
        let mut col = lerp(top, bottom, yf);

        let neb = ((time * 0.08 + yf * 8.0).sin() + 1.0) * 0.5;
        col = blend(col, (35, 10, 50), neb * 0.045);

        for x in 0..w {
            pix[y][x] = col;
        }
    }

    for i in 0..420usize {
        let sx = (i.wrapping_mul(2654435761) >> 4) % w.max(1);
        let sy = (i.wrapping_mul(2246822519) >> 4) % ph.max(1);
        let twinkle = ((time * 1.8 + i as f64 * 0.51).sin() + 1.0) * 0.5;

        if twinkle > 0.72 {
            let b = (120.0 + twinkle * 110.0) as u8;
            pix[sy][sx] = blend(pix[sy][sx], (b, b, 235), 0.55);
        }
    }
}

fn paint_lensing_field(pix: &mut Pix, w: usize, ph: usize, cx: f64, cy: f64, time: f64) {
    let max_r = w.min(ph) as f64 * 0.48;

    for y in 0..ph {
        for x in 0..w {
            let dx = x as f64 - cx;
            let dy = y as f64 - cy;
            let d = (dx * dx + dy * dy).sqrt();

            if d < max_r {
                let ring = ((d * 0.18 - time * 1.8).sin() + 1.0) * 0.5;
                let gravity = (1.0 - d / max_r).powf(1.5);
                let col = if ring > 0.75 { (60, 30, 95) } else { (20, 14, 45) };
                pix[y][x] = blend(pix[y][x], col, gravity * 0.22);
            }
        }
    }
}

fn paint_dust(pix: &mut Pix, w: usize, ph: usize, d: &Dust, cx: f64, cy: f64, scale: f64, time: f64) {
    let spiral = d.angle + d.radius * 0.035 + time * 0.06;
    let wobble = (time * 0.9 + d.phase).sin() * 2.0;

    let x = cx + spiral.cos() * d.radius * scale;
    let y = cy + spiral.sin() * d.radius * scale * 0.62 + wobble;

    let px = x.round() as i32;
    let py = y.round() as i32;

    if px < 0 || px >= w as i32 || py < 0 || py >= ph as i32 {
        return;
    }

    let energy = (1.0 - d.radius / 110.0).clamp(0.0, 1.0);
    let col = lerp((60, 80, 130), (180, 255, 220), energy);
    pix[py as usize][px as usize] = blend(pix[py as usize][px as usize], col, 0.25 + energy * 0.45);
}

fn paint_comet(pix: &mut Pix, w: usize, ph: usize, comet: &Comet) {
    if comet.life <= 0.0 {
        return;
    }

    let fade = (comet.life / comet.max_life).clamp(0.0, 1.0);

    for i in 0..24 {
        let t = i as f64 / 24.0;
        let x = comet.x - comet.vx * t * 0.045;
        let y = comet.y - comet.vy * t * 0.045;

        let alpha = fade * (1.0 - t).powf(1.35);
        paint_soft_circle(pix, w, ph, x, y, 2.2 * (1.0 - t * 0.45), comet.color, alpha * 0.65);
    }
}

fn paint_pod(pix: &mut Pix, w: usize, ph: usize, pod: &Pod, cx: f64, cy: f64, scale: f64, time: f64) {
    let breathe = ((pod.phase + time * 1.4).sin() + 1.0) * 0.5;
    let r = pod.orbit_r * scale;
    let x = cx + pod.angle.cos() * r;
    let y = cy + pod.angle.sin() * r * 0.62;

    // Draw thin orbital line hints.
    let orbit_alpha = 0.055 + breathe * 0.025;
    paint_orbit_hint(pix, w, ph, cx, cy, r, pod.color, orbit_alpha);

    // Draw flower/pod.
    let petals = 6;
    let petal_r = pod.size * scale * (1.0 + breathe * 0.28);

    for i in 0..petals {
        let a = TAU * i as f64 / petals as f64 + pod.phase * 0.20;
        let px = x + a.cos() * petal_r * 0.9;
        let py = y + a.sin() * petal_r * 0.55;
        paint_soft_circle(pix, w, ph, px, py, petal_r * 0.72, pod.color, 0.45);
    }

    paint_soft_circle(pix, w, ph, x, y, petal_r * 0.82, (245, 245, 210), 0.58);
    paint_soft_circle(pix, w, ph, x, y, petal_r * 1.8, pod.color, 0.08 + breathe * 0.10);
}

fn paint_orbit_hint(pix: &mut Pix, w: usize, ph: usize, cx: f64, cy: f64, r: f64, col: Rgb, alpha: f64) {
    let steps = (r * 5.0).clamp(40.0, 260.0) as usize;

    for i in 0..steps {
        if i % 3 != 0 {
            continue;
        }

        let a = TAU * i as f64 / steps as f64;
        let x = cx + a.cos() * r;
        let y = cy + a.sin() * r * 0.62;

        let px = x.round() as i32;
        let py = y.round() as i32;

        if px >= 0 && px < w as i32 && py >= 0 && py < ph as i32 {
            pix[py as usize][px as usize] = blend(pix[py as usize][px as usize], col, alpha);
        }
    }
}

fn paint_core(pix: &mut Pix, w: usize, ph: usize, cx: f64, cy: f64, scale: f64, time: f64) {
    let core_r = 9.0 * scale;
    let pulse = ((time * 1.5).sin() + 1.0) * 0.5;

    paint_soft_circle(pix, w, ph, cx, cy, core_r * 4.2, (110, 50, 180), 0.10 + pulse * 0.06);
    paint_ring(pix, w, ph, cx, cy, core_r * 2.2, (130, 255, 220), 0.28);
    paint_ring(pix, w, ph, cx, cy, core_r * 3.3 + pulse * 2.0, (255, 130, 220), 0.14);
    paint_soft_circle(pix, w, ph, cx, cy, core_r * 1.55, (20, 5, 35), 0.95);
    paint_soft_circle(pix, w, ph, cx, cy, core_r * 0.65, (0, 0, 5), 1.0);

    // Accretion slash.
    for i in -28..=28 {
        let x = cx + i as f64 * scale * 0.55;
        let y = cy + (i as f64 * 0.22 + time.sin() * 2.0) * scale * 0.35;
        paint_soft_circle(pix, w, ph, x, y, 1.4 * scale, (250, 180, 90), 0.35);
    }
}

fn paint_ring(pix: &mut Pix, w: usize, ph: usize, cx: f64, cy: f64, r: f64, col: Rgb, alpha: f64) {
    let min_x = (cx - r - 2.0).floor() as i32;
    let max_x = (cx + r + 2.0).ceil() as i32;
    let min_y = (cy - r - 2.0).floor() as i32;
    let max_y = (cy + r + 2.0).ceil() as i32;

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            if px < 0 || px >= w as i32 || py < 0 || py >= ph as i32 {
                continue;
            }

            let dx = px as f64 - cx;
            let dy = (py as f64 - cy) / 0.62;
            let d = (dx * dx + dy * dy).sqrt();
            let band = 1.0 - ((d - r).abs() / 1.5).clamp(0.0, 1.0);

            if band > 0.0 {
                pix[py as usize][px as usize] =
                    blend(pix[py as usize][px as usize], col, band * alpha);
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
                let a = (1.0 - d / r.max(1.0)).powf(1.45) * power;
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
