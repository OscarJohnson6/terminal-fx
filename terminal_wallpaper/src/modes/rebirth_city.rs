// ===== src/modes/rebirth_city.rs =====
//
// RebirthCityMode
//
// A dramatic simulation loop:
//   1. A procedural city grows from the ground.
//   2. Traffic/lights flicker while clouds drift.
//   3. A bomber plane crosses the sky.
//   4. A bright blast/shockwave destroys the city.
//   5. Fire, smoke, ash, and rubble fill the scene.
//   6. Rain arrives.
//   7. Trees, grass, and new buildings slowly regrow.
//   8. The city gets regenerated with a different layout.
//
// This mode is stylized terminal art, not an accurate weapon model.
// The focus is visual storytelling: destruction -> decay -> regrowth.
//
// Rendering:
//   Uses half-block RGB rendering. Each terminal row represents two pixel rows.

use crate::ansi::{bg_rgb, rgb, RESET};
use crate::color::ColorProvider;
use crate::mode_base::Mode;
use rand::RngExt;
use std::f64::consts::TAU;

type Rgb = (u8, u8, u8);
type Pix = Vec<Vec<Rgb>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Growing,
    Peace,
    Plane,
    Blast,
    Ruins,
    Rain,
    Regrowth,
}

#[derive(Clone, Copy)]
enum Roof {
    Flat,
    Antenna,
    Spire,
    Dome,
}

#[derive(Clone, Copy)]
struct Building {
    x: i32,
    w: i32,
    h: i32,
    target_h: i32,
    damage: f64,
    seed: f64,
    tint: Rgb,
    roof: Roof,
}

#[derive(Clone, Copy)]
struct Tree {
    x: f64,
    height: f64,
    target_height: f64,
    seed: f64,
}

#[derive(Clone, Copy)]
struct Cloud {
    x: f64,
    y_frac: f64,
    rx: f64,
    ry: f64,
    speed: f64,
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
    Fire,
    Smoke,
    Ash,
    Rubble,
    Rain,
}

pub struct RebirthCityMode {
    speed: f64,
    color_provider: ColorProvider,
    time: f64,
    phase_time: f64,
    phase: Phase,
    city_seed: u64,
    buildings: Vec<Building>,
    trees: Vec<Tree>,
    clouds: Vec<Cloud>,
    particles: Vec<Particle>,
    plane_x: f64,
    plane_y: f64,
    blast_x: f64,
    blast_y: f64,
    blast_radius: f64,
    flash: f64,
    last_dims: Option<(u16, u16)>,
}

impl RebirthCityMode {
    pub fn new(speed: f64, color_provider: ColorProvider) -> Self {
        let mut rng = rand::rng();

        Self {
            speed,
            color_provider,
            time: 0.0,
            phase_time: 0.0,
            phase: Phase::Growing,
            city_seed: rng.random_range(0..u64::MAX),
            buildings: Vec::new(),
            trees: Vec::new(),
            clouds: Vec::new(),
            particles: Vec::new(),
            plane_x: -80.0,
            plane_y: 10.0,
            blast_x: 0.0,
            blast_y: 0.0,
            blast_radius: 0.0,
            flash: 0.0,
            last_dims: None,
        }
    }

    fn reset_scene(&mut self, width: u16, height: u16) {
        self.buildings.clear();
        self.trees.clear();
        self.clouds.clear();
        self.particles.clear();

        let mut rng = rand::rng();
        self.city_seed = rng.random_range(0..u64::MAX);

        let w = width.max(1) as i32;
        let ph = height.max(1) as i32 * 2;
        let ground = ground_y(ph);

        let mut x = 0;
        while x < w {
            let bw = rng.random_range(3..9);
            let bh = rng.random_range((ph as f64 * 0.14) as i32..(ph as f64 * 0.42) as i32).max(5);

            let tint = match rng.random_range(0..5) {
                0 => (36, 42, 55),
                1 => (45, 38, 54),
                2 => (34, 47, 56),
                3 => (50, 45, 40),
                _ => (30, 35, 48),
            };

            let roof = match rng.random_range(0..4) {
                0 => Roof::Flat,
                1 => Roof::Antenna,
                2 => Roof::Spire,
                _ => Roof::Dome,
            };

            self.buildings.push(Building {
                x,
                w: bw,
                h: 0,
                target_h: bh.min(ground - 3),
                damage: 0.0,
                seed: rng.random_range(0.0..10000.0),
                tint,
                roof,
            });

            x += bw + rng.random_range(0..2);
        }

        let tree_count = (w / 8).clamp(8, 28);
        for i in 0..tree_count {
            self.trees.push(Tree {
                x: (i as f64 + rng.random_range(0.15..0.85)) / tree_count as f64 * w as f64,
                height: 0.0,
                target_height: rng.random_range(6.0..18.0),
                seed: rng.random_range(0.0..1000.0),
            });
        }

        let cloud_count = (w / 16).clamp(4, 12);
        for _ in 0..cloud_count {
            self.clouds.push(Cloud {
                x: rng.random_range(-80.0..w as f64 + 80.0),
                y_frac: rng.random_range(0.05..0.38),
                rx: rng.random_range(12.0..42.0),
                ry: rng.random_range(3.0..8.0),
                speed: rng.random_range(1.2..4.8),
            });
        }

        self.plane_x = -80.0;
        self.plane_y = ph as f64 * 0.16;
        self.blast_x = w as f64 * rng.random_range(0.38..0.62);
        self.blast_y = ground as f64 - 4.0;
        self.blast_radius = 0.0;
        self.flash = 0.0;
        self.phase = Phase::Growing;
        self.phase_time = 0.0;
        self.last_dims = Some((width, height));
    }

    fn next_phase(&mut self, width: u16, height: u16) {
        self.phase_time = 0.0;

        self.phase = match self.phase {
            Phase::Growing => Phase::Peace,
            Phase::Peace => {
                self.plane_x = -80.0;
                self.plane_y = height.max(1) as f64 * 2.0 * 0.14;
                Phase::Plane
            }
            Phase::Plane => {
                self.flash = 1.0;
                self.blast_radius = 1.0;
                Phase::Blast
            }
            Phase::Blast => Phase::Ruins,
            Phase::Ruins => Phase::Rain,
            Phase::Rain => Phase::Regrowth,
            Phase::Regrowth => {
                self.reset_scene(width, height);
                Phase::Growing
            }
        };
    }

    fn spawn_particle(&mut self, x: f64, y: f64, kind: ParticleKind) {
        let mut rng = rand::rng();

        let (vx, vy, life, color) = match kind {
            ParticleKind::Fire => (
                rng.random_range(-4.0..4.0),
                rng.random_range(-18.0..-5.0),
                rng.random_range(0.25..0.75),
                match rng.random_range(0..3) {
                    0 => (255, 205, 90),
                    1 => (255, 110, 45),
                    _ => (210, 45, 30),
                },
            ),
            ParticleKind::Smoke => (
                rng.random_range(-5.0..5.0),
                rng.random_range(-9.0..-2.0),
                rng.random_range(1.1..2.6),
                (75, 72, 78),
            ),
            ParticleKind::Ash => (
                rng.random_range(-2.5..2.5),
                rng.random_range(-4.0..1.0),
                rng.random_range(1.0..2.8),
                (120, 110, 100),
            ),
            ParticleKind::Rubble => (
                rng.random_range(-12.0..12.0),
                rng.random_range(-22.0..-5.0),
                rng.random_range(0.8..2.2),
                (95, 85, 76),
            ),
            ParticleKind::Rain => (
                rng.random_range(-3.5..-0.5),
                rng.random_range(35.0..60.0),
                rng.random_range(0.5..1.3),
                (110, 165, 190),
            ),
        };

        self.particles.push(Particle {
            x,
            y,
            vx,
            vy,
            life,
            max_life: life,
            color,
            kind,
        });
    }

    fn apply_blast_damage(&mut self) {
        for b in &mut self.buildings {
            let cx = b.x as f64 + b.w as f64 * 0.5;
            let top = self.blast_y - b.h as f64;
            let dx = cx - self.blast_x;
            let dy = top - self.blast_y;
            let dist = (dx * dx + dy * dy).sqrt();

            let pressure = (1.0 - dist / self.blast_radius.max(1.0)).clamp(0.0, 1.0);

            if pressure > 0.0 {
                b.damage = (b.damage + pressure * 0.12).clamp(0.0, 1.0);
                let collapse = (b.target_h as f64 * pressure * 0.25) as i32;
                b.h = (b.h - collapse).max(0);
            }
        }
    }
}

impl Mode for RebirthCityMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        if self.last_dims != Some((width, height)) || self.buildings.is_empty() {
            self.reset_scene(width, height);
        }

        let dt = (dt * self.speed).min(0.05);
        self.time += dt;
        self.phase_time += dt;

        let w = width.max(1) as f64;
        let ph = height.max(1) as f64 * 2.0;
        let ground = ground_y(ph as i32) as f64;

        for c in &mut self.clouds {
            c.x += c.speed * dt;
            if c.x > w + c.rx * 2.0 {
                c.x = -c.rx * 2.0;
            }
        }

        match self.phase {
            Phase::Growing => {
                for b in &mut self.buildings {
                    if b.h < b.target_h {
                        b.h += (18.0 * dt).ceil() as i32;
                        b.h = b.h.min(b.target_h);
                    }
                    b.damage = (b.damage - dt * 0.08).max(0.0);
                }

                if self.phase_time > 8.0 {
                    self.next_phase(width, height);
                }
            }
            Phase::Peace => {
                if self.phase_time > 8.0 {
                    self.next_phase(width, height);
                }
            }
            Phase::Plane => {
                self.plane_x += 34.0 * dt;

                if self.plane_x > self.blast_x {
                    self.next_phase(width, height);
                }
            }
            Phase::Blast => {
                self.flash = (self.flash - dt * 1.8).max(0.0);
                self.blast_radius += 62.0 * dt;
                self.apply_blast_damage();

                let mut rng = rand::rng();
                for _ in 0..12 {
                    let angle = rng.random_range(0.0..TAU);
                    let r = self.blast_radius * rng.random_range(0.2..1.0);
                    let x = self.blast_x + angle.cos() * r;
                    let y = self.blast_y + angle.sin() * r * 0.45;
                    self.spawn_particle(x, y, ParticleKind::Fire);
                    if rng.random_range(0..2) == 0 {
                        self.spawn_particle(x, y, ParticleKind::Rubble);
                    }
                }

                if self.phase_time > 2.2 {
                    self.next_phase(width, height);
                }
            }
            Phase::Ruins => {
                let mut rng = rand::rng();

                for _ in 0..8 {
                    let x = rng.random_range(0.0..w);
                    let y = rng.random_range(ground - 32.0..ground);
                    if rng.random_range(0..3) != 0 {
                        self.spawn_particle(x, y, ParticleKind::Smoke);
                    } else {
                        self.spawn_particle(x, y, ParticleKind::Ash);
                    }
                }

                for b in &mut self.buildings {
                    b.damage = (b.damage + dt * 0.04).clamp(0.0, 1.0);
                }

                if self.phase_time > 8.0 {
                    self.next_phase(width, height);
                }
            }
            Phase::Rain => {
                let mut rng = rand::rng();

                for _ in 0..24 {
                    self.spawn_particle(rng.random_range(0.0..w), rng.random_range(-8.0..2.0), ParticleKind::Rain);
                }

                for b in &mut self.buildings {
                    b.damage = (b.damage - dt * 0.10).max(0.35);
                }

                if self.phase_time > 8.0 {
                    self.next_phase(width, height);
                }
            }
            Phase::Regrowth => {
                for tree in &mut self.trees {
                    if tree.height < tree.target_height {
                        tree.height += 7.5 * dt;
                        tree.height = tree.height.min(tree.target_height);
                    }
                }

                for b in &mut self.buildings {
                    if b.h < b.target_h {
                        b.h += (8.0 * dt).ceil() as i32;
                        b.h = b.h.min(b.target_h);
                    }
                    b.damage = (b.damage - dt * 0.16).max(0.0);
                }

                if self.phase_time > 13.0 {
                    self.next_phase(width, height);
                }
            }
        }

        for p in &mut self.particles {
            p.x += p.vx * dt;
            p.y += p.vy * dt;

            match p.kind {
                ParticleKind::Fire => {
                    p.vy -= 3.0 * dt;
                    p.vx *= 0.985;
                }
                ParticleKind::Smoke => {
                    p.vy -= 2.0 * dt;
                    p.vx += (self.time * 1.3 + p.x * 0.02).sin() * dt * 1.2;
                }
                ParticleKind::Ash => {
                    p.vy += 2.2 * dt;
                    p.vx += (self.time * 2.0 + p.y * 0.03).sin() * dt * 1.0;
                }
                ParticleKind::Rubble => {
                    p.vy += 35.0 * dt;
                    if p.y > ground {
                        p.y = ground;
                        p.vy = 0.0;
                        p.vx *= 0.2;
                    }
                }
                ParticleKind::Rain => {}
            }

            p.life -= dt;
        }

        self.particles.retain(|p| {
            p.life > 0.0 && p.x > -10.0 && p.x < w + 10.0 && p.y > -20.0 && p.y < ph + 10.0
        });

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

        paint_sky(&mut pix, w, ph, self.phase, self.time, self.flash);

        for c in &self.clouds {
            paint_cloud(&mut pix, w, ph, c, self.phase);
        }

        paint_ground(&mut pix, w, ph, self.phase, self.time);

        for tree in &self.trees {
            paint_tree(&mut pix, w, ph, tree, self.time);
        }

        for b in &self.buildings {
            paint_building(&mut pix, w, ph, b, self.time);
        }

        if matches!(self.phase, Phase::Plane | Phase::Blast) {
            paint_plane(&mut pix, w, ph, self.plane_x, self.plane_y, self.time);
        }

        if matches!(self.phase, Phase::Blast | Phase::Ruins) {
            paint_blast(&mut pix, w, ph, self.blast_x, self.blast_y, self.blast_radius, self.flash);
        }

        for p in &self.particles {
            paint_particle(&mut pix, w, ph, p);
        }

        if self.flash > 0.01 {
            paint_flash(&mut pix, w, ph, self.flash);
        }

        half_blocks(&pix, w, h, &self.color_provider, t_abs)
    }
}

// ── Render helpers ────────────────────────────────────────────────────────────

fn ground_y(ph: i32) -> i32 {
    (ph as f64 * 0.78) as i32
}

fn paint_sky(pix: &mut Pix, w: usize, ph: usize, phase: Phase, time: f64, flash: f64) {
    let ruined = matches!(phase, Phase::Blast | Phase::Ruins);
    let rainy = matches!(phase, Phase::Rain);
    let green = matches!(phase, Phase::Regrowth);

    let top = if ruined {
        (42, 32, 35)
    } else if rainy {
        (18, 28, 42)
    } else if green {
        (20, 65, 68)
    } else {
        (16, 55, 105)
    };

    let bottom = if ruined {
        (115, 70, 52)
    } else if rainy {
        (55, 72, 82)
    } else if green {
        (90, 135, 92)
    } else {
        (120, 185, 225)
    };

    for y in 0..ph {
        let yf = y as f64 / ph.max(1) as f64;
        let mut col = lerp(top, bottom, yf.powf(0.76));

        let pulse = ((time * 0.16 + yf * 6.0).sin() + 1.0) * 0.5;
        col = blend(col, (255, 160, 80), flash * 0.35);
        col = blend(col, (80, 90, 120), pulse * 0.035);

        for x in 0..w {
            pix[y][x] = col;
        }
    }
}

fn paint_cloud(pix: &mut Pix, w: usize, ph: usize, cloud: &Cloud, phase: Phase) {
    let color = if matches!(phase, Phase::Blast | Phase::Ruins) {
        (86, 78, 80)
    } else if matches!(phase, Phase::Rain) {
        (95, 105, 118)
    } else {
        (230, 235, 242)
    };

    let cx = cloud.x as i32;
    let cy = (cloud.y_frac * ph as f64) as i32;
    let rx = cloud.rx as i32;
    let ry = cloud.ry as i32;

    for bump in -2..=2 {
        let bx = cx + bump * rx / 4;
        let brx = (rx as f64 * (0.42 + 0.08 * bump.abs() as f64)) as i32;
        let bry = (ry as f64 * (0.8 + 0.08 * bump.abs() as f64)) as i32;

        for dy in -bry..=bry {
            for dx in -brx..=brx {
                let ex = dx as f64 / brx.max(1) as f64;
                let ey = dy as f64 / bry.max(1) as f64;
                let d = ex * ex + ey * ey;

                if d <= 1.0 {
                    set_blend(pix, w, ph, bx + dx, cy + dy, color, (1.0 - d).powf(0.55) * 0.55);
                }
            }
        }
    }
}

fn paint_ground(pix: &mut Pix, w: usize, ph: usize, phase: Phase, time: f64) {
    let ground = ground_y(ph as i32).max(0) as usize;

    let base = if matches!(phase, Phase::Blast | Phase::Ruins) {
        (52, 44, 38)
    } else if matches!(phase, Phase::Rain) {
        (42, 58, 48)
    } else if matches!(phase, Phase::Regrowth) {
        (35, 105, 45)
    } else {
        (45, 82, 42)
    };

    let deep = if matches!(phase, Phase::Regrowth) {
        (20, 70, 32)
    } else {
        (30, 30, 28)
    };

    for y in ground..ph {
        let depth = (y - ground) as f64 / (ph - ground).max(1) as f64;
        for x in 0..w {
            let shimmer = ((x as f64 * 0.18 + time * 1.4).sin() + 1.0) * 0.5;
            let col = blend(lerp(base, deep, depth), (80, 130, 70), shimmer * 0.04);
            pix[y][x] = col;
        }
    }
}

fn paint_building(pix: &mut Pix, w: usize, ph: usize, b: &Building, time: f64) {
    if b.h <= 0 {
        return;
    }

    let ground = ground_y(ph as i32);
    let top = ground - b.h;
    let collapse_cut = (b.damage * b.h as f64 * 0.45) as i32;

    for y in top + collapse_cut..=ground {
        for x in b.x..b.x + b.w {
            if x < 0 || x >= w as i32 || y < 0 || y >= ph as i32 {
                continue;
            }

            let edge = x == b.x || x == b.x + b.w - 1;
            let noise = hash01(b.seed + x as f64 * 9.1 + y as f64 * 3.7);
            let broken = noise < b.damage * 0.55;

            if broken {
                continue;
            }

            let mut col = b.tint;
            if edge {
                col = darken(col, 0.18);
            }
            col = blend(col, (24, 22, 24), b.damage * 0.58);

            pix[y as usize][x as usize] = col;

            let window_row = y % 5 == 0;
            let window_col = x % 3 == 0;
            if window_row && window_col && b.damage < 0.82 {
                let flicker = ((time * 2.2 + b.seed + x as f64).sin() + 1.0) * 0.5;
                if flicker > 0.35 && noise > 0.22 {
                    let light = if b.damage > 0.25 { (255, 105, 45) } else { (255, 215, 110) };
                    pix[y as usize][x as usize] = blend(pix[y as usize][x as usize], light, 0.78);
                }
            }
        }
    }

    if b.damage < 0.70 {
        paint_roof(pix, w, ph, b, top + collapse_cut, time);
    }
}

fn paint_roof(pix: &mut Pix, w: usize, ph: usize, b: &Building, top: i32, time: f64) {
    let cx = b.x + b.w / 2;

    match b.roof {
        Roof::Flat => {
            for x in b.x..b.x + b.w {
                set_blend(pix, w, ph, x, top - 1, brighten(b.tint, 18), 0.9);
            }
        }
        Roof::Antenna => {
            for y in top - 7..top {
                set_blend(pix, w, ph, cx, y, (70, 72, 80), 0.9);
            }

            let pulse = ((time * 4.0 + b.seed).sin() + 1.0) * 0.5;
            paint_soft_circle(pix, w, ph, cx as f64, (top - 8) as f64, 3.0, (255, 60, 90), pulse * 0.34);
        }
        Roof::Spire => {
            for i in 0..6 {
                for dx in -i / 2..=i / 2 {
                    set_blend(pix, w, ph, cx + dx, top - i, brighten(b.tint, 30), 0.85);
                }
            }
        }
        Roof::Dome => {
            let r = (b.w / 2).max(2);
            for dy in -r..=0 {
                for dx in -r..=r {
                    let d = (dx * dx + dy * dy) as f64 / (r * r).max(1) as f64;
                    if d <= 1.0 {
                        set_blend(pix, w, ph, cx + dx, top + dy, brighten(b.tint, 22), 0.84);
                    }
                }
            }
        }
    }
}

fn paint_tree(pix: &mut Pix, w: usize, ph: usize, tree: &Tree, time: f64) {
    if tree.height <= 0.1 {
        return;
    }

    let ground = ground_y(ph as i32) as f64;
    let x = tree.x.round() as i32;
    let h = tree.height as i32;

    let sway = ((time * 1.1 + tree.seed).sin() * 1.5) as i32;

    for i in 0..h {
        let y = ground as i32 - i;
        set_blend(pix, w, ph, x, y, (68, 45, 24), 0.9);
    }

    let canopy_y = ground as i32 - h;
    let r = (tree.height * 0.42).clamp(2.5, 7.0) as i32;

    for dy in -r..=r {
        for dx in -r * 2..=r * 2 {
            let ex = dx as f64 / (r * 2).max(1) as f64;
            let ey = dy as f64 / r.max(1) as f64;
            let d = ex * ex + ey * ey;
            if d <= 1.0 {
                let col = blend((45, 125, 50), (95, 175, 70), 1.0 - d);
                set_blend(pix, w, ph, x + dx + sway, canopy_y + dy, col, 0.92);
            }
        }
    }
}

fn paint_plane(pix: &mut Pix, w: usize, ph: usize, x: f64, y: f64, time: f64) {
    let x = x.round() as i32;
    let y = y.round() as i32;
    let body = (45, 48, 55);
    let light = ((time * 8.0).sin() + 1.0) * 0.5;

    for dx in -9..=9 {
        set_blend(pix, w, ph, x + dx, y, body, 0.95);
    }
    for dx in -4..=5 {
        set_blend(pix, w, ph, x + dx, y - 1, brighten(body, 25), 0.9);
    }
    for dy in -3..=3 {
        set_blend(pix, w, ph, x - 1, y + dy, body, 0.88);
    }
    for dx in -8..=-3 {
        set_blend(pix, w, ph, x + dx, y + 2, darken(body, 0.2), 0.9);
    }

    paint_soft_circle(pix, w, ph, (x + 10) as f64, y as f64, 2.4, (255, 70, 70), light * 0.45);
}

fn paint_blast(pix: &mut Pix, w: usize, ph: usize, x: f64, y: f64, radius: f64, flash: f64) {
    if radius <= 0.0 {
        return;
    }

    paint_soft_circle(pix, w, ph, x, y, radius * 0.32, (255, 245, 185), 0.75 + flash * 0.3);
    paint_soft_circle(pix, w, ph, x, y, radius * 0.70, (255, 135, 55), 0.28);
    paint_ring(pix, w, ph, x, y, radius, (255, 230, 170), 0.62);
    paint_ring(pix, w, ph, x, y, radius * 0.65, (255, 100, 70), 0.32);
}

fn paint_particle(pix: &mut Pix, w: usize, ph: usize, p: &Particle) {
    let fade = (p.life / p.max_life).clamp(0.0, 1.0);

    match p.kind {
        ParticleKind::Rain => {
            for i in 0..4 {
                set_blend(
                    pix,
                    w,
                    ph,
                    p.x.round() as i32 - i,
                    p.y.round() as i32 + i,
                    p.color,
                    fade * (0.38 - i as f64 * 0.06),
                );
            }
        }
        ParticleKind::Smoke => {
            paint_soft_circle(pix, w, ph, p.x, p.y, 3.8 * (1.0 - fade + 0.25), p.color, fade * 0.32);
        }
        ParticleKind::Fire => {
            paint_soft_circle(pix, w, ph, p.x, p.y, 2.2, p.color, fade * 0.72);
        }
        ParticleKind::Ash => {
            paint_soft_circle(pix, w, ph, p.x, p.y, 1.2, p.color, fade * 0.32);
        }
        ParticleKind::Rubble => {
            paint_soft_circle(pix, w, ph, p.x, p.y, 1.5, p.color, fade * 0.80);
        }
    }
}

fn paint_flash(pix: &mut Pix, w: usize, ph: usize, flash: f64) {
    for y in 0..ph {
        for x in 0..w {
            pix[y][x] = blend(pix[y][x], (255, 240, 200), flash * 0.45);
        }
    }
}

fn paint_ring(pix: &mut Pix, w: usize, ph: usize, cx: f64, cy: f64, r: f64, col: Rgb, alpha: f64) {
    let min_x = (cx - r - 2.0).floor() as i32;
    let max_x = (cx + r + 2.0).ceil() as i32;
    let min_y = (cy - r * 0.55 - 2.0).floor() as i32;
    let max_y = (cy + r * 0.55 + 2.0).ceil() as i32;

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            if px < 0 || px >= w as i32 || py < 0 || py >= ph as i32 {
                continue;
            }

            let dx = px as f64 - cx;
            let dy = (py as f64 - cy) / 0.55;
            let d = (dx * dx + dy * dy).sqrt();
            let band = 1.0 - ((d - r).abs() / 1.8).clamp(0.0, 1.0);

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
                let a = (1.0 - d / r.max(1.0)).powf(1.4) * power;
                pix[py as usize][px as usize] =
                    blend(pix[py as usize][px as usize], col, a);
            }
        }
    }
}

fn set_blend(pix: &mut Pix, w: usize, ph: usize, x: i32, y: i32, col: Rgb, alpha: f64) {
    if x >= 0 && x < w as i32 && y >= 0 && y < ph as i32 {
        pix[y as usize][x as usize] = blend(pix[y as usize][x as usize], col, alpha);
    }
}

fn hash01(n: f64) -> f64 {
    (n.sin() * 43758.5453123).fract().abs()
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
