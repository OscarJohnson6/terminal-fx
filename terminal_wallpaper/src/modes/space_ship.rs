// ===== src/modes/space_ship.rs =====
//
// PREDICTIVE PATHFINDING:
// Asteroids move on a fixed timer. Before every BFS we build TWO obstacle
// grids — one for where asteroids are RIGHT NOW, one for where they will be
// AFTER their next move — and mark any cell blocked in either grid as off-
// limits. The ship therefore never enters a cell that will be dangerous on
// the next asteroid tick. The "warning glow" on an asteroid whose timer is
// < 0.25s gives the user (and ship) a visual heads-up.
//
// HALF-BLOCK RENDERING:
// Asteroids and the ship are drawn into a pixel buffer using ▀ (U+2580)
// so we get 2× vertical resolution. The star background and HUD text are
// still drawn as normal characters on top.

use crate::ansi::{RESET};
use crate::color::ColorProvider;
use crate::mode_base::Mode;
use rand::RngExt;
use std::collections::VecDeque;

type Rgb = (u8, u8, u8);

// ── Helpers ───────────────────────────────────────────────────────────────────

fn lerp_rgb(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let c = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * t) as u8;
    (c(a.0, b.0), c(a.1, b.1), c(a.2, b.2))
}

fn blend_rgb(bg: Rgb, fg: Rgb, a: f64) -> Rgb { lerp_rgb(bg, fg, a) }

// ── Data ──────────────────────────────────────────────────────────────────────

struct Star { x: usize, y: usize, phase: f64, bright: f64 }

struct Asteroid {
    // Float position for smooth display; integer grid positions for BFS.
    fx: f64, fy: f64,
    gx: i32, gy: i32,  // current grid cell
    ngx: i32, ngy: i32, // predicted next grid cell
    vgx: i32, vgy: i32, // grid velocity (cells per tick)
    move_interval: f64,
    move_timer: f64,
    radius: f64,
    seed: u32,
}

struct Alien {
    id: u32,
    fx: f64, fy: f64,
    vx: f64, vy: f64,
    turn_timer: f64,
}

struct Particle {
    x: f64, y: f64,
    vx: f64, vy: f64,
    life: f64, max_life: f64,
    col: Rgb,
}

// ── Mode ──────────────────────────────────────────────────────────────────────

pub struct SpaceShipMode {
    color_provider: ColorProvider,
    speed: f64,
    stars: Vec<Star>,
    asteroids: Vec<Asteroid>,
    aliens: Vec<Alien>,
    particles: Vec<Particle>,
    ship_fx: f64, ship_fy: f64,
    ship_dx: f64, ship_dy: f64, // facing direction (unit-ish)
    path: Vec<(usize, usize)>,
    path_step: usize,
    path_timer: f64,
    score: u32,
    next_id: u32,
    initialized: bool,
}

impl SpaceShipMode {
    pub fn new(speed: f64, color_provider: ColorProvider) -> Self {
        Self {
            color_provider, speed,
            stars: Vec::new(), asteroids: Vec::new(),
            aliens: Vec::new(), particles: Vec::new(),
            ship_fx: 5.0, ship_fy: 5.0,
            ship_dx: 1.0, ship_dy: 0.0,
            path: Vec::new(), path_step: 0, path_timer: 0.0,
            score: 0, next_id: 0, initialized: false,
        }
    }

    fn init(&mut self, w: u16, h: u16) {
        let mut rng = rand::rng();
        self.stars = (0..180).map(|_| Star {
            x:      rng.random_range(0..w as usize),
            y:      rng.random_range(0..h as usize),
            phase:  rng.random_range(0.0..std::f64::consts::TAU),
            bright: rng.random_range(0.3..1.0),
        }).collect();

        self.asteroids = (0..14).map(|_| {
            let gx = rng.random_range(6..w as i32 - 6);
            let gy = rng.random_range(3..h as i32 - 3);
            // Asteroid moves at most 1 cell per tick, in a random direction
            let dirs = [(1,0),(-1,0),(0,1),(0,-1),(1,1),(1,-1),(-1,1),(-1,-1)];
            let (vgx, vgy) = dirs[rng.random_range(0..dirs.len())];
            let interval = rng.random_range(0.4..0.9);
            Asteroid {
                fx: gx as f64, fy: gy as f64,
                gx, gy, ngx: gx + vgx, ngy: gy + vgy,
                vgx, vgy, move_interval: interval,
                move_timer: rng.random_range(0.0..interval),
                radius: rng.random_range(1.5..3.0),
                seed: rng.random_range(0..10000),
            }
        }).collect();

        self.aliens.clear();
        for _ in 0..4 { self.spawn_alien(w, h, &mut rand::rng()); }
        self.ship_fx = 4.0; self.ship_fy = 4.0;
        self.initialized = true;
    }

    fn spawn_alien(&mut self, w: u16, h: u16, rng: &mut impl RngExt) {
        let angle = rng.random_range(0.0..std::f64::consts::TAU);
        let spd   = rng.random_range(2.0..4.5) * self.speed;
        let id = self.next_id; self.next_id += 1;
        self.aliens.push(Alien {
            id,
            fx: rng.random_range(6.0..w as f64 - 6.0),
            fy: rng.random_range(2.0..h as f64 - 2.0),
            vx: angle.cos() * spd,
            vy: angle.sin() * spd * 0.45,
            turn_timer: rng.random_range(1.0..3.5),
        });
    }

    fn rebuild_path(&mut self, w: u16, h: u16) {
        let wi = w as usize;
        let hi = h as usize;

        // Find nearest alien
        let target = match self.aliens.iter().min_by(|a, b| {
            let da = (a.fx - self.ship_fx).powi(2) + (a.fy - self.ship_fy).powi(2);
            let db = (b.fx - self.ship_fx).powi(2) + (b.fy - self.ship_fy).powi(2);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        }) {
            Some(a) => ((a.fx.round() as usize).min(wi-1), (a.fy.round() as usize).min(hi-1)),
            None    => { self.path.clear(); return; }
        };

        let start = (
            (self.ship_fx.round() as usize).min(wi-1),
            (self.ship_fy.round() as usize).min(hi-1),
        );

        // Build obstacle grid: block current AND predicted asteroid positions
        let mut grid = vec![vec![false; wi]; hi];
        for ast in &self.asteroids {
            let r = (ast.radius as i32) + 1;
            // Mark current position
            mark_circle(&mut grid, wi, hi, ast.gx, ast.gy, r);
            // Mark predicted next position — this is the key predictive step
            mark_circle(&mut grid, wi, hi, ast.ngx, ast.ngy, r);
        }
        grid[start.1][start.0]    = false; // ship start always passable
        grid[target.1][target.0]  = false; // alien target always passable

        self.path      = bfs(&grid, wi, hi, start, target);
        self.path_step = 0;
    }
}

fn mark_circle(grid: &mut Vec<Vec<bool>>, w: usize, h: usize, cx: i32, cy: i32, r: i32) {
    for dy in -r..=r {
        for dx in -r..=r {
            // Aspect-ratio correction: terminal cells are ~2× taller
            let fx = dx as f64 / r as f64;
            let fy = (dy as f64 / r as f64) * 0.5;
            if fx*fx + fy*fy <= 1.0 {
                let nx = cx + dx; let ny = cy + dy;
                if nx >= 0 && nx < w as i32 && ny >= 0 && ny < h as i32 {
                    grid[ny as usize][nx as usize] = true;
                }
            }
        }
    }
}

fn bfs(grid: &[Vec<bool>], w: usize, h: usize,
       start: (usize, usize), goal: (usize, usize)) -> Vec<(usize, usize)>
{
    if start == goal { return vec![]; }
    let mut prev    = vec![vec![Option::<(usize,usize)>::None; w]; h];
    let mut visited = vec![vec![false; w]; h];
    let mut queue   = VecDeque::new();
    visited[start.1][start.0] = true;
    queue.push_back(start);
    const DIRS: &[(i32,i32)] = &[(1,0),(-1,0),(0,1),(0,-1),(1,1),(1,-1),(-1,1),(-1,-1)];
    'outer: while let Some((cx,cy)) = queue.pop_front() {
        for &(ddx,ddy) in DIRS {
            let nx = cx as i32 + ddx; let ny = cy as i32 + ddy;
            if nx<0||nx>=w as i32||ny<0||ny>=h as i32 { continue; }
            let (nx,ny) = (nx as usize, ny as usize);
            if grid[ny][nx] || visited[ny][nx] { continue; }
            visited[ny][nx] = true;
            prev[ny][nx] = Some((cx,cy));
            if (nx,ny)==goal { break 'outer; }
            queue.push_back((nx,ny));
        }
    }
    if !visited[goal.1][goal.0] { return vec![]; }
    let mut path = Vec::new();
    let mut cur  = goal;
    while cur != start {
        path.push(cur);
        match prev[cur.1][cur.0] { Some(p) => cur = p, None => break }
    }
    path.reverse();
    path
}

impl Mode for SpaceShipMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t: f64) {
        if !self.initialized { self.init(width, height); return; }
        let dt = dt * self.speed;
        let (w, h) = (width as f64, height as f64);
        let mut rng = rand::rng();

        // ── Tick asteroids ────────────────────────────────────────────────
        for ast in &mut self.asteroids {
            ast.move_timer -= dt;
            if ast.move_timer <= 0.0 {
                // Execute the predicted move
                ast.gx = ast.ngx; ast.gy = ast.ngy;
                ast.fx = ast.gx as f64; ast.fy = ast.gy as f64;
                // Bounce off walls
                if ast.gx <= 2 || ast.gx >= width as i32 - 3 { ast.vgx = -ast.vgx; }
                if ast.gy <= 1 || ast.gy >= height as i32 - 2 { ast.vgy = -ast.vgy; }
                // Compute next predicted position for the BFS to avoid
                ast.ngx = (ast.gx + ast.vgx).clamp(1, width as i32 - 2);
                ast.ngy = (ast.gy + ast.vgy).clamp(0, height as i32 - 1);
                ast.move_timer = ast.move_interval;
            }
        }

        // ── Move aliens ───────────────────────────────────────────────────
        for alien in &mut self.aliens {
            alien.fx += alien.vx * dt; alien.fy += alien.vy * dt;
            alien.turn_timer -= dt;
            if alien.fx < 2.0 || alien.fx > w-3.0 { alien.vx = -alien.vx; alien.fx = alien.fx.clamp(2.0,w-3.0); }
            if alien.fy < 1.0 || alien.fy > h-2.0 { alien.vy = -alien.vy; alien.fy = alien.fy.clamp(1.0,h-2.0); }
            if alien.turn_timer <= 0.0 {
                let ang = rng.random_range(0.0..std::f64::consts::TAU);
                let spd = rng.random_range(1.5..4.0) * self.speed;
                alien.vx = ang.cos()*spd; alien.vy = ang.sin()*spd*0.45;
                alien.turn_timer = rng.random_range(1.5..4.5);
            }
        }

        // ── Pathfind & move ship ──────────────────────────────────────────
        self.path_timer -= dt;
        if self.path_timer <= 0.0 || self.path.is_empty() || self.path_step >= self.path.len() {
            self.rebuild_path(width, height);
            self.path_timer = 0.55;
        }

        let ship_spd = 12.0 * self.speed;
        if self.path_step < self.path.len() {
            let (wx,wy) = self.path[self.path_step];
            let dx = wx as f64 - self.ship_fx;
            let dy = wy as f64 - self.ship_fy;
            let dist = (dx*dx+dy*dy).sqrt();
            if dist < 0.3 {
                self.path_step += 1;
            } else {
                let mv = (ship_spd * dt).min(dist);
                self.ship_fx += (dx/dist)*mv; self.ship_fy += (dy/dist)*mv;
                self.ship_dx = dx/dist; self.ship_dy = dy/dist;
            }
        }
        self.ship_fx = self.ship_fx.clamp(1.0, w-2.0);
        self.ship_fy = self.ship_fy.clamp(0.5, h-1.5);

        // Thruster particles
        let (tdx, tdy) = (-self.ship_dx, -self.ship_dy);
        if rng.random_bool(0.6) {
            let spread = rng.random_range(-0.4..0.4);
            let spd = rng.random_range(4.0..10.0);
            self.particles.push(Particle {
                x: self.ship_fx + tdx, y: self.ship_fy + tdy,
                vx: (tdx + spread) * spd, vy: (tdy + spread*0.5) * spd,
                life: rng.random_range(0.08..0.25), max_life: 0.25,
                col: (255, rng.random_range(80u8..200u8), 0),
            });
        }

        // ── Kill aliens on contact ─────────────────────────────────────────
        let mut killed = Vec::new();
        for alien in &self.aliens {
            let dx = alien.fx - self.ship_fx; let dy = alien.fy - self.ship_fy;
            if dx*dx + dy*dy < 2.5*2.5 { killed.push(alien.id); }
        }
        for id in killed {
            if let Some(i) = self.aliens.iter().position(|a| a.id==id) {
                let a = self.aliens.remove(i);
                self.score += 1;
                for _ in 0..28 {
                    let ang = rng.random_range(0.0..std::f64::consts::TAU);
                    let spd = rng.random_range(4.0..14.0);
                    let life = rng.random_range(0.3..0.8);
                    self.particles.push(Particle {
                        x: a.fx, y: a.fy,
                        vx: ang.cos()*spd, vy: ang.sin()*spd*0.45,
                        life, max_life: life,
                        col: [(255,200,0),(255,100,0),(255,50,50)][rng.random_range(0..3)],
                    });
                }
                if self.aliens.len() < 3 { self.spawn_alien(width, height, &mut rng); }
                self.path.clear();
            }
        }

        // Tick + cull particles
        for p in &mut self.particles { p.x+=p.vx*dt; p.y+=p.vy*dt; p.life-=dt; }
        self.particles.retain(|p| p.life > 0.0);
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w = width  as usize;
        let h = height as usize;
        let ph = h * 2; // pixel rows (half-block doubled resolution)

        // Pixel buffer: (r,g,b) per pixel
        let mut pix = vec![vec![(0u8, 0u8, 18u8); w]; ph];

        // ── Stars (pixel layer) ────────────────────────────────────────────
        for s in &self.stars {
            let py = s.y * 2;
            if s.x < w && py < ph {
                let b = ((t_abs * 0.8 + s.phase).sin() * 0.5 + 0.5) * s.bright;
                if b > 0.15 {
                    let v = (b * 200.0 + 30.0) as u8;
                    pix[py][s.x] = (v, v, (v as f64 * 0.85) as u8);
                }
            }
        }

        // ── BFS path debug (faint dotted trail) ────────────────────────────
        if self.path_step < self.path.len() {
            for &(px, py) in &self.path[self.path_step..] {
                let ppx = py * 2;
                if px < w && ppx < ph {
                    pix[ppx][px] = blend_rgb(pix[ppx][px], (30, 60, 90), 0.6);
                }
            }
        }

        // ── Asteroids (pixel layer) ────────────────────────────────────────
        for ast in &self.asteroids {
            let r = ast.radius;
            let ax = ast.fx; let ay = ast.fy;
            // Warning glow when about to move
            let warn = (ast.move_timer / ast.move_interval).clamp(0.0, 1.0);
            let glow_col = lerp_rgb((100,60,30), (255, 80, 0), 1.0 - warn);

            for dy in -(r as i32 + 1)..=(r as i32 + 1) {
                for dx in -(r as i32 + 1)..=(r as i32 + 1) {
                    let fx = dx as f64 / r; let fy = (dy as f64 / r) * 0.5;
                    let d  = (fx*fx + fy*fy).sqrt();
                    if d > 1.15 { continue; }
                    let px = (ax + dx as f64).round() as i32;
                    let py = ((ay + dy as f64) * 2.0).round() as i32;
                    if px < 0 || px >= w as i32 || py < 0 || py >= ph as i32 { continue; }
                    let (px, py) = (px as usize, py as usize);
                    let rock_col = if d <= 1.0 {
                        // Core: grey rock with edge darkening
                        let edge = d.powi(2);
                        let base = ((1.0 - edge) * 145.0 + 40.0) as u8;
                        let r = base.saturating_sub(10); let g = r; let b = r.saturating_sub(20);
                        // Subtle variation from seed
                        let noise = ((ast.seed as f64 * 0.01 + px as f64 * 0.3 + py as f64 * 0.2).sin() * 20.0) as i32;
                        let r = (r as i32 + noise).clamp(20, 200) as u8;
                        (r, g, b)
                    } else {
                        // Warning glow ring
                        let a = (1.0 - (d - 1.0) / 0.15).clamp(0.0, 1.0) * (1.0 - warn);
                        blend_rgb(pix[py][px], glow_col, a * 0.7)
                    };
                    pix[py][px] = rock_col;
                }
            }

            // Predicted position indicator (faint ghost outline when asteroid is about to move)
            if warn < 0.3 {
                let alpha = (1.0 - warn / 0.3) * 0.35;
                let nx = ast.ngx as f64; let ny = ast.ngy as f64;
                for dy in -(r as i32)..=(r as i32) {
                    for dx in -(r as i32)..=(r as i32) {
                        let fx = dx as f64 / r; let fy = (dy as f64 / r) * 0.5;
                        if (fx*fx + fy*fy - 1.0).abs() < 0.15 { // ring only
                            let px = (nx + dx as f64).round() as i32;
                            let py = ((ny + dy as f64) * 2.0).round() as i32;
                            if px>=0&&px<w as i32&&py>=0&&py<ph as i32 {
                                pix[py as usize][px as usize] =
                                    blend_rgb(pix[py as usize][px as usize], (255,80,0), alpha);
                            }
                        }
                    }
                }
            }
        }

        // ── Particles ─────────────────────────────────────────────────────
        for p in &self.particles {
            let px = p.x.round() as i32; let py = (p.y * 2.0).round() as i32;
            if px>=0&&px<w as i32&&py>=0&&py<ph as i32 {
                let fade = (p.life / p.max_life).clamp(0.0,1.0);
                let col  = lerp_rgb((0,0,0), p.col, fade);
                pix[py as usize][px as usize] = col;
            }
        }

        // ── Aliens ────────────────────────────────────────────────────────
        for alien in &self.aliens {
            let ax = alien.fx.round() as i32;
            let ay = (alien.fy * 2.0).round() as i32;
            let pulse = ((t_abs * 3.5 + alien.id as f64 * 1.9).sin() * 0.5 + 0.5) as f64;
            let acol  = ((20.0+pulse*30.0) as u8, (170.0+pulse*85.0) as u8, (40.0+pulse*20.0) as u8);
            // Draw a small saucer shape in pixel coords
            for (ddx, ddy) in &[(0,0),(1,0),(-1,0),(0,-1),(0,1),(2,0),(-2,0)] {
                let px = ax+ddx; let py = ay+ddy;
                if px>=0&&px<w as i32&&py>=0&&py<ph as i32 {
                    pix[py as usize][px as usize] = acol;
                }
            }
        }

        // ── Ship ──────────────────────────────────────────────────────────
        let sx  = self.ship_fx.round() as i32;
        let spy = (self.ship_fy * 2.0).round() as i32;
        // Ship color from ColorProvider (mapped to pixel Rgb)
        let ship_col: Rgb = {
            let t = (t_abs * 2.0).rem_euclid(1.0);
            let r = ((t * std::f64::consts::TAU).sin() * 60.0 + 190.0) as u8;
            let g = ((t * std::f64::consts::TAU + 2.0).sin() * 40.0 + 200.0) as u8;
            let b = 255;
            (r, g, b)
        };
        let ship_points: &[(i32,i32)] = &[
            (0,0),(1,0),(-1,0),(0,-1),(0,1),(2,0),(0,-2),
        ];
        for &(ddx, ddy) in ship_points {
            let px = sx+ddx; let py = spy+ddy;
            if px>=0&&px<w as i32&&py>=0&&py<ph as i32 {
                pix[py as usize][px as usize] = ship_col;
            }
        }

        // ── Compose half-blocks ────────────────────────────────────────────
        let mut out = String::with_capacity(w * h * 40);
        for row in 0..h {
            let upper = row * 2;
            let lower = row * 2 + 1;
            for col in 0..w {
                let (ur,ug,ub) = pix[upper][col];
                let (lr,lg,lb) = if lower < ph { pix[lower][col] } else { (0,0,0) };
                out.push_str(&format!(
                    "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m\u{2580}",
                    ur,ug,ub, lr,lg,lb
                ));
            }
            out.push_str(RESET);
            if row < h-1 { out.push('\n'); }
        }

        // ── Overlay HUD (normal chars on top) ─────────────────────────────
        // Write HUD as separate lines appended after the pixel frame.
        // Because we use HOME to reposition, these overwrite the first screen row.
        // Instead, encode them into the first line by inserting ANSI position codes.
        // Simpler: return the pixel frame; main.rs can add an overlay. For now,
        // inject score into the rendered string via cursor positioning.
        let hud_score   = format!(" SCORE {:03} ", self.score);
        let hud_status  = if self.path.is_empty() { " SCANNING " } else { " HUNTING  " };
        let hud_line    = format!(
            "\x1b[1;1H\x1b[38;2;80;200;120m{}\x1b[38;2;255;220;80m{}\x1b[38;2;255;220;80m KILLS{}\x1b[1;{}H{}{}",
            hud_status, " ".repeat(w.saturating_sub(hud_status.len() + hud_score.len() + 6 + 6)),
            " ", w.saturating_sub(hud_score.len()), hud_score, RESET
        );
        format!("{}{}", out, hud_line)
    }
}