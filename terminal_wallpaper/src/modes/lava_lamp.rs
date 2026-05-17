// ===== src/modes/lava_lamp.rs =====
//
// LavaLampMode
//
// A smooth lava lamp wallpaper mode.
// This is intentionally different from a generic metaballs field:
//   - Visible glass lamp capsule.
//   - Wax blobs heat up near the bottom, rise, cool near the top, then sink.
//   - Small bubbles and shimmer particles.
//   - Heating coil / base glow.
//   - Theme-aware color palette without full-screen rainbow wash.
//
// Suggested registry:
//
// In src/modes/mod.rs:
//   pub mod lava_lamp;
//
// In mode_registry.rs descriptor imports:
//   lava_lamp::LavaLampMode,
//
// Descriptor:
//   impl ModeDescriptor for LavaLampMode {
//       const ID:   &'static str = "lava_lamp";
//       const NAME: &'static str = "Lava Lamp";
//       const DESC: &'static str = "Wax blobs rising inside glass";
//       const FPS:  u32          = 50;
//   }
//
// In register_all!:
//   lava_lamp::LavaLampMode,

use crate::ansi::{bg_rgb, rgb, RESET};
use crate::color::{ColorMode, ColorProvider};
use crate::mode_base::Mode;
use rand::RngExt;
use std::f64::consts::TAU;

type Rgb = (u8, u8, u8);
type Pix = Vec<Vec<Rgb>>;

#[derive(Clone, Copy)]
struct Blob {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    rx: f64,
    ry: f64,
    heat: f64,
    phase: f64,
    color: Rgb,
    split_timer: f64,
}

#[derive(Clone, Copy)]
struct Bubble {
    x: f64,
    y: f64,
    r: f64,
    vy: f64,
    phase: f64,
    life: f64,
    max_life: f64,
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
struct LampGeom {
    cx: f64,
    top: f64,
    bottom: f64,
    max_w: f64,
    neck_w: f64,
    base_y: f64,
}

pub struct LavaLampMode {
    speed: f64,
    color_provider: ColorProvider,

    time: f64,
    blobs: Vec<Blob>,
    bubbles: Vec<Bubble>,
    sparks: Vec<Spark>,

    last_dims: Option<(u16, u16)>,
}

impl LavaLampMode {
    pub fn new(speed: f64, color_provider: ColorProvider) -> Self {
        Self {
            speed,
            color_provider,
            time: 0.0,
            blobs: Vec::new(),
            bubbles: Vec::new(),
            sparks: Vec::new(),
            last_dims: None,
        }
    }

    fn reset_scene(&mut self, width: u16, height: u16) {
        let mut rng = rand::rng();
        let geom = lamp_geom(width, height);

        self.time = 0.0;
        self.blobs.clear();
        self.bubbles.clear();
        self.sparks.clear();

        let blob_count = if width < 80 { 9 } else { 14 };

        for i in 0..blob_count {
            let y_frac = rng.random_range(0.08..0.92);
            let y = geom.top + (geom.bottom - geom.top) * y_frac;
            let half_w = lamp_half_width(&geom, y) * 0.72;

            let palette = wax_palette(self.color_provider.mode);
            let color = palette[rng.random_range(0..palette.len())];

            self.blobs.push(Blob {
                x: geom.cx + rng.random_range(-half_w..half_w),
                y,
                vx: rng.random_range(-2.0..2.0),
                vy: rng.random_range(-7.0..7.0),
                rx: rng.random_range(3.5..8.5),
                ry: rng.random_range(3.0..9.5),
                heat: rng.random_range(0.15..0.95),
                phase: rng.random_range(0.0..TAU) + i as f64,
                color,
                split_timer: rng.random_range(4.0..13.0),
            });
        }

        for _ in 0..18 {
            self.spawn_bubble(width, height);
        }

        self.last_dims = Some((width, height));
    }

    fn spawn_bubble(&mut self, width: u16, height: u16) {
        if self.bubbles.len() > 80 {
            return;
        }

        let mut rng = rand::rng();
        let geom = lamp_geom(width, height);
        let y = rng.random_range(geom.bottom - 7.0..geom.bottom - 1.0);
        let half_w = lamp_half_width(&geom, y) * 0.55;

        let life = rng.random_range(2.2..6.0);

        self.bubbles.push(Bubble {
            x: geom.cx + rng.random_range(-half_w..half_w),
            y,
            r: rng.random_range(0.6..1.7),
            vy: rng.random_range(5.0..12.0),
            phase: rng.random_range(0.0..TAU),
            life,
            max_life: life,
        });
    }

    fn spawn_spark(&mut self, x: f64, y: f64, color: Rgb) {
        if self.sparks.len() > 130 {
            return;
        }

        let mut rng = rand::rng();
        let life = rng.random_range(0.35..1.1);

        self.sparks.push(Spark {
            x,
            y,
            vx: rng.random_range(-3.0..3.0),
            vy: rng.random_range(-8.0..-2.0),
            life,
            max_life: life,
            color,
        });
    }
}

impl Mode for LavaLampMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        if self.last_dims != Some((width, height)) || self.blobs.is_empty() {
            self.reset_scene(width, height);
        }

        let dt = (dt * self.speed).min(0.05);
        self.time += dt;

        let geom = lamp_geom(width, height);
        let lamp_h = (geom.bottom - geom.top).max(1.0);
        let mut rng = rand::rng();

        for i in 0..self.blobs.len() {
            let can_split = self.blobs.len() < 24;
            let mut split_now = false;
            let mut spark_at: Option<(f64, f64, Rgb)> = None;

            {
                let blob = &mut self.blobs[i];

                let depth = ((blob.y - geom.top) / lamp_h).clamp(0.0, 1.0);

                // Heat rises near the bottom and fades near the top.
                let bottom_heat = smoothstep(0.46, 1.0, depth);
                let top_cool = 1.0 - smoothstep(0.0, 0.40, depth);

                blob.heat += (bottom_heat * 0.75 - top_cool * 0.38) * dt;
                blob.heat += ((self.time * 0.75 + blob.phase).sin()) * dt * 0.035;
                blob.heat = blob.heat.clamp(0.0, 1.25);

                // Hot wax rises; cool wax falls.
                let buoyancy = (blob.heat - 0.53) * -18.0;
                blob.vy += buoyancy * dt;
                blob.vy += (self.time * 1.2 + blob.phase).sin() * dt * 2.3;
                blob.vy *= 0.992;

                blob.vx += (self.time * 0.9 + blob.y * 0.05 + blob.phase).sin() * dt * 3.0;
                blob.vx *= 0.985;

                blob.x += blob.vx * dt;
                blob.y += blob.vy * dt;

                // Soft shape breathing.
                let breathe = ((self.time * 1.2 + blob.phase).sin() + 1.0) * 0.5;
                let heat_stretch = blob.heat.clamp(0.0, 1.0);
                blob.ry += ((3.0 + heat_stretch * 6.5 + breathe * 1.2) - blob.ry) * dt * 0.7;
                blob.rx += ((4.0 + (1.0 - heat_stretch) * 4.4 + (1.0 - breathe) * 1.0) - blob.rx) * dt * 0.6;

                // Keep inside glass.
                let half_w = lamp_half_width(&geom, blob.y).max(3.0);
                let left = geom.cx - half_w + blob.rx * 0.45;
                let right = geom.cx + half_w - blob.rx * 0.45;

                if blob.x < left {
                    blob.x = left;
                    blob.vx = blob.vx.abs() * 0.55;
                } else if blob.x > right {
                    blob.x = right;
                    blob.vx = -blob.vx.abs() * 0.55;
                }

                if blob.y < geom.top + blob.ry * 0.65 {
                    blob.y = geom.top + blob.ry * 0.65;
                    blob.vy = blob.vy.abs() * 0.52;
                    blob.heat *= 0.70;
                } else if blob.y > geom.bottom - blob.ry * 0.65 {
                    blob.y = geom.bottom - blob.ry * 0.65;
                    blob.vy = -blob.vy.abs() * 0.42;
                    blob.heat = (blob.heat + 0.18).min(1.25);
                }

                blob.split_timer -= dt;

                if blob.split_timer <= 0.0 && blob.rx > 5.0 && can_split {
                    split_now = true;
                    blob.split_timer = rng.random_range(6.0..15.0);
                    blob.rx *= 0.80;
                    blob.ry *= 0.86;
                    spark_at = Some((blob.x, blob.y, blob.color));
                }
            }

            if let Some((x, y, c)) = spark_at {
                for _ in 0..4 {
                    self.spawn_spark(x, y, c);
                }
            }

            if split_now {
                let source = self.blobs[i];
                self.blobs.push(Blob {
                    x: source.x + rng.random_range(-2.5..2.5),
                    y: source.y + rng.random_range(-2.5..2.5),
                    vx: -source.vx * 0.6 + rng.random_range(-2.0..2.0),
                    vy: source.vy * 0.4 + rng.random_range(-3.0..3.0),
                    rx: (source.rx * rng.random_range(0.55..0.75)).max(2.6),
                    ry: (source.ry * rng.random_range(0.50..0.78)).max(2.8),
                    heat: source.heat * rng.random_range(0.86..1.05),
                    phase: rng.random_range(0.0..TAU),
                    color: source.color,
                    split_timer: rng.random_range(6.0..14.0),
                });
            }
        }

        // Merge tiny nearby blobs so the lamp does not become noisy forever.
        let mut i = 0usize;
        while i < self.blobs.len() {
            let mut j = i + 1;
            while j < self.blobs.len() {
                let dx = self.blobs[i].x - self.blobs[j].x;
                let dy = self.blobs[i].y - self.blobs[j].y;
                let dist2 = dx * dx + dy * dy;
                let merge_r = (self.blobs[i].rx + self.blobs[j].rx) * 0.38;

                if dist2 < merge_r * merge_r && self.blobs.len() > 9 {
                    let a = self.blobs[i];
                    let b = self.blobs[j];
                    let mass_a = (a.rx * a.ry).max(1.0);
                    let mass_b = (b.rx * b.ry).max(1.0);
                    let total = mass_a + mass_b;

                    self.blobs[i] = Blob {
                        x: (a.x * mass_a + b.x * mass_b) / total,
                        y: (a.y * mass_a + b.y * mass_b) / total,
                        vx: (a.vx * mass_a + b.vx * mass_b) / total,
                        vy: (a.vy * mass_a + b.vy * mass_b) / total,
                        rx: (a.rx.powi(2) + b.rx.powi(2)).sqrt().clamp(3.0, 10.5),
                        ry: (a.ry.powi(2) + b.ry.powi(2)).sqrt().clamp(3.0, 12.5),
                        heat: (a.heat * mass_a + b.heat * mass_b) / total,
                        phase: a.phase,
                        color: blend(a.color, b.color, 0.5),
                        split_timer: rng.random_range(7.0..16.0),
                    };

                    self.blobs.remove(j);
                } else {
                    j += 1;
                }
            }
            i += 1;
        }

        if rng.random_range(0.0..1.0) < 0.25 {
            self.spawn_bubble(width, height);
        }

        for bubble in &mut self.bubbles {
            bubble.phase += dt * 3.0;
            bubble.y -= bubble.vy * dt;
            bubble.x += (self.time * 1.7 + bubble.phase).sin() * dt * 2.2;
            bubble.life -= dt;

            let half_w = lamp_half_width(&geom, bubble.y).max(1.0);
            bubble.x = bubble.x.clamp(geom.cx - half_w * 0.82, geom.cx + half_w * 0.82);

            if bubble.y < geom.top + 2.0 {
                bubble.life = 0.0;
            }
        }

        self.bubbles.retain(|b| b.life > 0.0);

        let heat_color = heat_color_for_mode(self.color_provider.mode);
        if rng.random_range(0.0..1.0) < 0.18 {
            self.spawn_spark(
                geom.cx + rng.random_range(-geom.max_w * 0.25..geom.max_w * 0.25),
                geom.bottom + rng.random_range(-2.0..2.0),
                heat_color,
            );
        }

        for spark in &mut self.sparks {
            spark.x += spark.vx * dt;
            spark.y += spark.vy * dt;
            spark.vy -= 0.2 * dt;
            spark.vx *= 0.97;
            spark.life -= dt;
        }

        self.sparks.retain(|s| s.life > 0.0);
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w = width.max(1) as usize;
        let h = height.max(1) as usize;
        let ph = h * 2;

        let geom = lamp_geom(width, height);
        let mut pix: Pix = vec![vec![(0, 0, 0); w]; ph];

        paint_background(&mut pix, w, ph, self.color_provider.mode, self.time);
        paint_lamp_glow(&mut pix, w, ph, &geom, self.color_provider.mode, self.time);
        paint_lamp_liquid(&mut pix, w, ph, &geom, self.color_provider.mode, self.time);

        for blob in &self.blobs {
            paint_blob(&mut pix, w, ph, &geom, *blob, self.time);
        }

        for bubble in &self.bubbles {
            paint_bubble(&mut pix, w, ph, &geom, *bubble);
        }

        for spark in &self.sparks {
            paint_spark(&mut pix, w, ph, *spark);
        }

        paint_glass(&mut pix, w, ph, &geom, self.time);
        paint_base_and_cap(&mut pix, w, ph, &geom, self.color_provider.mode, self.time);
        paint_table_reflection(&mut pix, w, ph, &geom, self.color_provider.mode, self.time);
        paint_vignette(&mut pix, w, ph);

        half_blocks(&pix, w, h, &self.color_provider, t_abs)
    }
}

// ── Geometry ─────────────────────────────────────────────────────────────────

fn lamp_geom(width: u16, height: u16) -> LampGeom {
    let w = width.max(1) as f64;
    let ph = height.max(1) as f64 * 2.0;

    let max_w = (w * 0.34).clamp(18.0, 42.0);
    let neck_w = max_w * 0.36;
    let top = ph * 0.13;
    let bottom = ph * 0.78;
    let base_y = ph * 0.86;

    LampGeom {
        cx: w * 0.5,
        top,
        bottom,
        max_w,
        neck_w,
        base_y,
    }
}

fn lamp_half_width(g: &LampGeom, y: f64) -> f64 {
    let t = ((y - g.top) / (g.bottom - g.top).max(1.0)).clamp(0.0, 1.0);

    // Neck near top/bottom, bulbous middle.
    let bulb = (std::f64::consts::PI * t).sin().powf(0.55);
    let taper = g.neck_w * 0.5 + (g.max_w * 0.5 - g.neck_w * 0.5) * bulb;
    taper.max(2.0)
}

// ── Painting ─────────────────────────────────────────────────────────────────

fn paint_background(pix: &mut Pix, w: usize, ph: usize, mode: ColorMode, time: f64) {
    let (top, bottom) = match mode {
        ColorMode::Ocean => ((4, 16, 30), (14, 42, 65)),
        ColorMode::Sunset => ((40, 16, 26), (92, 44, 32)),
        ColorMode::Matrix => ((0, 10, 6), (6, 34, 16)),
        ColorMode::Rainbow => ((12, 10, 28), (44, 30, 62)),
    };

    for y in 0..ph {
        let yf = y as f64 / ph.max(1) as f64;
        let mut col = lerp(top, bottom, yf.powf(0.85));

        let wall_wave = ((time * 0.18 + y as f64 * 0.065).sin() + 1.0) * 0.5;
        let accent = match mode {
            ColorMode::Ocean => (40, 105, 140),
            ColorMode::Sunset => (180, 75, 45),
            ColorMode::Matrix => (20, 125, 50),
            ColorMode::Rainbow => (150, 65, 210),
        };

        col = blend(col, accent, wall_wave * 0.035);

        for x in 0..w {
            pix[y][x] = col;
        }
    }

    // Subtle wallpaper stripes.
    for x in (0..w).step_by(9) {
        for y in 0..ph {
            pix[y][x] = blend(pix[y][x], (255, 255, 255), 0.012);
        }
    }
}

fn paint_lamp_glow(pix: &mut Pix, w: usize, ph: usize, g: &LampGeom, mode: ColorMode, time: f64) {
    let heat = heat_color_for_mode(mode);
    let pulse = ((time * 0.9).sin() + 1.0) * 0.5;

    paint_soft_ellipse(
        pix,
        w,
        ph,
        g.cx,
        g.bottom,
        g.max_w * 0.70,
        (g.bottom - g.top) * 0.44,
        heat,
        0.060 + pulse * 0.035,
    );

    paint_soft_ellipse(
        pix,
        w,
        ph,
        g.cx,
        g.bottom + 2.0,
        g.max_w * 0.50,
        11.0,
        heat,
        0.11 + pulse * 0.05,
    );
}

fn paint_lamp_liquid(pix: &mut Pix, w: usize, ph: usize, g: &LampGeom, mode: ColorMode, time: f64) {
    let liquid = liquid_color_for_mode(mode);
    let top = g.top as i32;
    let bottom = g.bottom as i32;

    for y in top..=bottom {
        let half_w = lamp_half_width(g, y as f64);
        let left = (g.cx - half_w).floor() as i32;
        let right = (g.cx + half_w).ceil() as i32;

        for x in left..=right {
            if !in_bounds(w, ph, x, y) {
                continue;
            }

            let nx = ((x as f64 - g.cx) / half_w.max(1.0)).abs();
            if nx > 1.0 {
                continue;
            }

            let yf = ((y as f64 - g.top) / (g.bottom - g.top).max(1.0)).clamp(0.0, 1.0);
            let inner = (1.0 - nx.powf(2.5)).clamp(0.0, 1.0);
            let swirl = ((time * 0.5 + x as f64 * 0.05 + y as f64 * 0.025).sin() + 1.0) * 0.5;

            let mut col = blend(liquid, (255, 255, 255), inner * 0.055);
            col = blend(col, heat_color_for_mode(mode), yf.powf(2.0) * 0.14);
            col = blend(col, (255, 255, 255), swirl * 0.020);

            pix[y as usize][x as usize] = blend(pix[y as usize][x as usize], col, 0.56);
        }
    }
}

fn paint_blob(pix: &mut Pix, w: usize, ph: usize, g: &LampGeom, blob: Blob, time: f64) {
    let wobble_x = 1.0 + ((time * 1.1 + blob.phase).sin()) * 0.12;
    let wobble_y = 1.0 + ((time * 1.3 + blob.phase + 1.7).sin()) * 0.16;

    let rx = blob.rx * wobble_x;
    let ry = blob.ry * wobble_y;

    let hot = heat_color_for_blob(blob.color);
    let col = blend(blob.color, hot, blob.heat.clamp(0.0, 1.0) * 0.38);

    paint_soft_ellipse(pix, w, ph, blob.x, blob.y, rx + 4.0, ry + 4.0, col, 0.10);
    paint_ellipse(pix, w, ph, blob.x, blob.y, rx, ry, col, 0.88);

    // Hot core / specular center.
    let pulse = ((time * 2.0 + blob.phase).sin() + 1.0) * 0.5;
    paint_soft_ellipse(
        pix,
        w,
        ph,
        blob.x - rx * 0.22,
        blob.y - ry * 0.22,
        rx * 0.36,
        ry * 0.30,
        (255, 235, 170),
        0.12 + pulse * 0.08,
    );

    // Keep edges inside glass visually by darkening outside the lamp.
    let half_w = lamp_half_width(g, blob.y);
    if (blob.x - g.cx).abs() > half_w {
        paint_soft_ellipse(pix, w, ph, blob.x, blob.y, rx, ry, (0, 0, 0), 0.4);
    }
}

fn paint_bubble(pix: &mut Pix, w: usize, ph: usize, _g: &LampGeom, b: Bubble) {
    let fade = (b.life / b.max_life).clamp(0.0, 1.0);
    let pulse = ((b.phase + b.life * 5.0).sin() + 1.0) * 0.5;

    paint_soft_circle(pix, w, ph, b.x, b.y, b.r + 1.0, (220, 240, 255), fade * 0.05);
    paint_ring(pix, w, ph, b.x, b.y, b.r * (0.8 + pulse * 0.18), (210, 235, 255), fade * 0.34);
}

fn paint_spark(pix: &mut Pix, w: usize, ph: usize, s: Spark) {
    let fade = (s.life / s.max_life).clamp(0.0, 1.0);

    paint_soft_circle(pix, w, ph, s.x, s.y, 1.2, s.color, fade * 0.52);
    paint_soft_circle(pix, w, ph, s.x, s.y, 4.0, s.color, fade * 0.055);
}

fn paint_glass(pix: &mut Pix, w: usize, ph: usize, g: &LampGeom, time: f64) {
    let top = g.top as i32;
    let bottom = g.bottom as i32;

    for y in top..=bottom {
        let half_w = lamp_half_width(g, y as f64);
        let left = (g.cx - half_w).round() as i32;
        let right = (g.cx + half_w).round() as i32;

        let edge_col = (195, 215, 230);
        set_blend(pix, w, ph, left, y, edge_col, 0.38);
        set_blend(pix, w, ph, right, y, edge_col, 0.38);

        if y % 3 == 0 {
            set_blend(pix, w, ph, left + 1, y, (255, 255, 255), 0.10);
            set_blend(pix, w, ph, right - 1, y, (255, 255, 255), 0.08);
        }
    }

    // Vertical glass highlights.
    for y in top + 3..bottom - 3 {
        let half_w = lamp_half_width(g, y as f64);
        let shimmer = ((time * 0.7 + y as f64 * 0.05).sin() + 1.0) * 0.5;
        let x1 = (g.cx - half_w * 0.55 + shimmer * 1.5).round() as i32;
        let x2 = (g.cx + half_w * 0.42).round() as i32;

        set_blend(pix, w, ph, x1, y, (255, 255, 255), 0.18);
        if y % 2 == 0 {
            set_blend(pix, w, ph, x2, y, (255, 255, 255), 0.07);
        }
    }
}

fn paint_base_and_cap(pix: &mut Pix, w: usize, ph: usize, g: &LampGeom, mode: ColorMode, time: f64) {
    let metal = match mode {
        ColorMode::Ocean => (58, 78, 90),
        ColorMode::Sunset => (105, 70, 48),
        ColorMode::Matrix => (35, 75, 45),
        ColorMode::Rainbow => (72, 62, 86),
    };

    let heat = heat_color_for_mode(mode);
    let pulse = ((time * 1.4).sin() + 1.0) * 0.5;

    // Cap.
    paint_rect(
        pix,
        w,
        ph,
        g.cx - g.neck_w * 0.62,
        g.top - 7.0,
        g.neck_w * 1.24,
        8.0,
        metal,
        0.84,
    );

    paint_rect(
        pix,
        w,
        ph,
        g.cx - g.neck_w * 0.78,
        g.top - 9.0,
        g.neck_w * 1.56,
        3.0,
        brighten(metal, 28),
        0.78,
    );

    // Base cone.
    let base_top = g.bottom - 2.0;
    let base_bottom = g.base_y;
    for y in base_top as i32..=base_bottom as i32 {
        let t = ((y as f64 - base_top) / (base_bottom - base_top).max(1.0)).clamp(0.0, 1.0);
        let half = g.neck_w * 0.65 + g.max_w * 0.34 * t;
        paint_rect(
            pix,
            w,
            ph,
            g.cx - half,
            y as f64,
            half * 2.0,
            1.0,
            blend(metal, (20, 18, 22), t * 0.25),
            0.88,
        );
    }

    // Heating coil.
    let coil_y = g.bottom + 1.5;
    for i in 0..18 {
        let x = g.cx - g.max_w * 0.28 + i as f64 * g.max_w * 0.56 / 17.0;
        let y = coil_y + (i as f64 * 1.4 + time * 4.0).sin() * 1.2;
        paint_soft_circle(pix, w, ph, x, y, 1.3, heat, 0.52 + pulse * 0.20);
        paint_soft_circle(pix, w, ph, x, y, 4.0, heat, 0.05);
    }
}

fn paint_table_reflection(pix: &mut Pix, w: usize, ph: usize, g: &LampGeom, mode: ColorMode, time: f64) {
    let table_y = (g.base_y + 5.0).min(ph as f64 - 2.0);
    let col = heat_color_for_mode(mode);

    paint_rect(pix, w, ph, 0.0, table_y, w as f64, ph as f64 - table_y, (18, 15, 20), 0.52);

    let shimmer = ((time * 1.1).sin() + 1.0) * 0.5;
    paint_soft_ellipse(
        pix,
        w,
        ph,
        g.cx,
        table_y + 2.0,
        g.max_w * (0.80 + shimmer * 0.12),
        4.0,
        col,
        0.11,
    );
}

fn paint_vignette(pix: &mut Pix, w: usize, ph: usize) {
    for y in 0..ph {
        for x in 0..w {
            let nx = (x as f64 / w.max(1) as f64 - 0.5).abs() * 2.0;
            let ny = (y as f64 / ph.max(1) as f64 - 0.5).abs() * 2.0;
            let d = ((nx * nx + ny * ny) * 0.5).clamp(0.0, 1.0);
            pix[y][x] = darken(pix[y][x], d.powf(2.25) * 0.34);
        }
    }
}

// ── Color palettes ───────────────────────────────────────────────────────────

fn wax_palette(mode: ColorMode) -> &'static [Rgb] {
    match mode {
        ColorMode::Ocean => &[
            (55, 210, 245),
            (60, 180, 230),
            (95, 240, 210),
            (80, 155, 255),
        ],
        ColorMode::Sunset => &[
            (255, 105, 55),
            (255, 155, 65),
            (235, 70, 80),
            (255, 195, 70),
        ],
        ColorMode::Matrix => &[
            (60, 255, 95),
            (25, 210, 70),
            (110, 255, 130),
            (35, 180, 55),
        ],
        ColorMode::Rainbow => &[
            (255, 70, 135),
            (245, 110, 235),
            (255, 155, 65),
            (115, 210, 255),
        ],
    }
}

fn liquid_color_for_mode(mode: ColorMode) -> Rgb {
    match mode {
        ColorMode::Ocean => (10, 42, 68),
        ColorMode::Sunset => (62, 22, 26),
        ColorMode::Matrix => (0, 30, 14),
        ColorMode::Rainbow => (38, 20, 55),
    }
}

fn heat_color_for_mode(mode: ColorMode) -> Rgb {
    match mode {
        ColorMode::Ocean => (75, 225, 255),
        ColorMode::Sunset => (255, 135, 55),
        ColorMode::Matrix => (85, 255, 105),
        ColorMode::Rainbow => (255, 90, 185),
    }
}

fn heat_color_for_blob(blob: Rgb) -> Rgb {
    blend(blob, (255, 230, 140), 0.45)
}

fn lamp_theme_tint(color_provider: &ColorProvider, base: Rgb, t: f64, x: i32, y: i32) -> Rgb {
    // Keep the actual wax/liquid palette readable. This is a soft grade,
    // not the full global rainbow tint used by older modes.
    match color_provider.mode {
        ColorMode::Rainbow => {
            let pulse = ((t * 0.55 + (x + y) as f64 * 0.018).sin() + 1.0) * 0.5;
            blend(base, (220, 80, 255), 0.030 + pulse * 0.030)
        }
        ColorMode::Ocean => blend(base, (50, 145, 190), 0.10),
        ColorMode::Sunset => blend(base, (255, 125, 65), 0.10),
        ColorMode::Matrix => blend(base, (35, 220, 85), 0.14),
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

fn paint_ellipse(pix: &mut Pix, w: usize, ph: usize, cx: f64, cy: f64, rx: f64, ry: f64, col: Rgb, power: f64) {
    let min_x = (cx - rx - 1.0).floor() as i32;
    let max_x = (cx + rx + 1.0).ceil() as i32;
    let min_y = (cy - ry - 1.0).floor() as i32;
    let max_y = (cy + ry + 1.0).ceil() as i32;

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            if !in_bounds(w, ph, px, py) {
                continue;
            }

            let dx = (px as f64 - cx) / rx.max(1.0);
            let dy = (py as f64 - cy) / ry.max(1.0);
            let d = dx * dx + dy * dy;

            if d <= 1.0 {
                let shade = 1.0 - d.sqrt();
                let lit = blend(col, (255, 255, 220), shade * 0.22);
                pix[py as usize][px as usize] =
                    blend(pix[py as usize][px as usize], lit, power);
            }
        }
    }
}

fn paint_soft_ellipse(pix: &mut Pix, w: usize, ph: usize, cx: f64, cy: f64, rx: f64, ry: f64, col: Rgb, power: f64) {
    let min_x = (cx - rx - 1.0).floor() as i32;
    let max_x = (cx + rx + 1.0).ceil() as i32;
    let min_y = (cy - ry - 1.0).floor() as i32;
    let max_y = (cy + ry + 1.0).ceil() as i32;

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            if !in_bounds(w, ph, px, py) {
                continue;
            }

            let dx = (px as f64 - cx) / rx.max(1.0);
            let dy = (py as f64 - cy) / ry.max(1.0);
            let d = (dx * dx + dy * dy).sqrt();

            if d <= 1.0 {
                let a = (1.0 - d).powf(1.5) * power;
                pix[py as usize][px as usize] =
                    blend(pix[py as usize][px as usize], col, a);
            }
        }
    }
}

fn paint_soft_circle(pix: &mut Pix, w: usize, ph: usize, cx: f64, cy: f64, r: f64, col: Rgb, power: f64) {
    paint_soft_ellipse(pix, w, ph, cx, cy, r, r, col, power);
}

fn paint_ring(pix: &mut Pix, w: usize, ph: usize, cx: f64, cy: f64, r: f64, col: Rgb, power: f64) {
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
            let edge = (d - r).abs();

            if edge <= 0.75 {
                let a = (1.0 - edge / 0.75).clamp(0.0, 1.0) * power;
                pix[py as usize][px as usize] =
                    blend(pix[py as usize][px as usize], col, a);
            }
        }
    }
}

fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = ((x - edge0) / (edge1 - edge0).max(0.0001)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
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

            let fg = lamp_theme_tint(color_provider, base_fg, t_abs, x as i32, upper as i32);
            let bg = lamp_theme_tint(color_provider, base_bg, t_abs, x as i32, lower as i32);

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
