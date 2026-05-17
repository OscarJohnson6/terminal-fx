// ===== modes/shooting_stars.rs =====
// Full port of shootingstars.py.
//
// This file demonstrates several important Rust patterns:
//   - Structs with Vec<T> fields (like Python lists)
//   - impl blocks for methods
//   - Ownership: Vec::retain() instead of rebuilding lists
//   - Option<T> for nullable fields
//   - rand crate for random numbers
 
use crate::ansi::{DIM, RESET};
use crate::color::ColorProvider;
use crate::mode_base::Mode;
use rand::{RngExt};

// ---------- Data structs ----------
// Rust `struct`s replace Python dicts. Each field has an explicit type.
// No runtime type errors — the compiler catches mismatches.
 
struct Star {
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,
    speed: f64,
    length: i32,
    seed: i32,
    depth: f64,
}
 
struct Particle {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    life: f64,
    seed: i32,
    depth: f64,
}
 
struct BgStar {
    x: usize,
    y: usize,
    phase: f64,
}
 
// ---------- Main mode struct ----------
pub struct ShootingStarMode {
    speed_factor: f64,
    color_provider: ColorProvider,
    explode_on_land: bool,
 
    // `Vec<T>` is Rust's growable array — equivalent to Python's `list`.
    // Unlike Python, every element must be the same type T.
    stars: Vec<Star>,
    explosions: Vec<Particle>,
 
    min_stars: usize,
    max_stars: usize,
 
    bg_stars: Vec<BgStar>,
    // `Option<(u16, u16)>` replaces Python's `Optional[tuple[int, int]]`.
    // Rust never lets you use a None value accidentally — you must unwrap it.
    bg_dims: Option<(u16, u16)>,
}

impl std::ops::Deref for ShootingStarMode {
    type Target = Option<(u16, u16)>;

    fn deref(&self) -> &Self::Target {
        &self.bg_dims
    }
}
 
impl ShootingStarMode {
    /// Public constructor. `Self` refers to `ShootingStarMode`.
    pub fn new(speed_factor: f64, color_provider: ColorProvider) -> Self {
        Self {
            speed_factor,
            color_provider,
            explode_on_land: true,
            stars: Vec::new(),
            explosions: Vec::new(),
            min_stars: 1,
            max_stars: 4,
            bg_stars: Vec::new(),
            bg_dims: None,
        }
    }
 
    fn spawn_star(&mut self, width: u16, height: u16) {
        let mut rng = rand::rng();
        let spawn_from_top = rng.random_bool(0.4);
 
        let (x, y) = if spawn_from_top {
            (
                rng.random_range(0.0..(width as f64 * 0.3).max(1.0)),
                rng.random_range(-8.0..0.0),
            )
        } else {
            (
                rng.random_range(-10.0..0.0),
                rng.random_range(-2.0..(height as f64 * 0.3)),
            )
        };
 
        let angle = rng.random_range(
            10.0_f64.to_radians()..30.0_f64.to_radians(),
        );
 
        let speed = rng.random_range(25.0..45.0) * self.speed_factor;
        let length = rng.random_range(4..=8);
        let depth = rng.random_range(0.4..1.0);
        let seed = rng.random_range(0..10000);
 
        self.stars.push(Star {
            x,
            y,
            dx: angle.cos(),
            dy: angle.sin(),
            speed,
            length,
            seed,
            depth,
        });
    }
 
    fn spawn_explosion(&mut self, x: f64, y: f64, seed: i32, depth: f64) {
        let mut rng = rand::rng();
        let num = ((30.0 * depth) as usize).max(8);
 
        for _ in 0..num {
            let angle = rng.random_range(0.0..(2.0 * std::f64::consts::PI));
            let speed = rng.random_range(10.0..28.0) * depth * self.speed_factor;
            let life = rng.random_range(0.18..0.5) * (0.7 + 0.6 * depth);
 
            self.explosions.push(Particle {
                x,
                y,
                vx: speed * angle.cos(),
                vy: speed * angle.sin() * 0.6,
                life,
                seed,
                depth,
            });
        }
    }
}
 
// ---------- Implement the Mode trait ----------
// This is the Rust equivalent of `class ShootingStarMode(ModeBase):`
// with the method bodies matching `update` and `render`.
 
impl Mode for ShootingStarMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        let mut rng = rand::rng();
 
        // Maintain star count
        if self.stars.len() < self.min_stars {
            self.spawn_star(width, height);
        } else if self.stars.len() < self.max_stars && rng.random_bool(0.02) {
            self.spawn_star(width, height);
        }
 
        // Move stars and collect explosions for those that exit.
        // We can't modify self.stars while calling self.spawn_explosion,
        // so we collect pending explosions first.
        // KEY RUST CONCEPT: the borrow checker prevents you from having a
        // mutable and immutable reference to the same data simultaneously.
        // Splitting the logic into "collect exits" then "spawn" is the pattern.
        let mut pending_explosions: Vec<(f64, f64, i32, f64)> = Vec::new();
 
        for star in &mut self.stars {
            star.x += star.dx * star.speed * dt;
            star.y += star.dy * star.speed * dt;
        }
 
        // `Vec::retain` keeps only elements where the closure returns true.
        // It's more efficient than Python's list comprehension rebuild because
        // it modifies the Vec in place without allocating a new one.
        let w = width as f64;
        let h = height as f64;
 
        self.stars.retain(|star| {
            if star.x >= w || star.y >= h {
                let exp_x = star.x.clamp(0.0, w - 1.0);
                let exp_y = star.y.clamp(0.0, h - 1.0);
                pending_explosions.push((exp_x, exp_y, star.seed, star.depth));
                false // remove from Vec
            } else if star.x < -20.0 || star.y < -20.0 {
                false
            } else {
                true // keep
            }
        });
 
        if self.explode_on_land {
            for (x, y, seed, depth) in pending_explosions {
                self.spawn_explosion(x, y, seed, depth);
            }
        }
 
        // Advance explosion particles; remove dead ones.
        for p in &mut self.explosions {
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.life -= dt;
        }
        self.explosions.retain(|p| p.life > 0.0);
    }
 
    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w = width as usize;
        let h = height as usize;
 
        // A 2D buffer of strings (one cell each).
        // `vec![expr; n]` creates a Vec of n copies of expr.
        // We use String here because each cell may hold ANSI codes + char.
        let mut buf: Vec<Vec<String>> = vec![vec![" ".to_string(); w]; h];
 
        // --- Background stars ---
        for s in &self.bg_stars {
            if s.x < w && s.y < h {
                let b = (((t_abs * 0.7 + s.phase).sin() + 1.0) / 2.0).clamp(0.0, 1.0);
                if b < 0.15 {
                    continue;
                }
                let (col, ch) = if b < 0.5 {
                    ("\x1b[38;5;244m", ".")
                } else {
                    ("\x1b[38;5;250m", "\u{00B7}") // middle dot ·
                };
                if buf[s.y][s.x] == " " {
                    buf[s.y][s.x] = format!("{}{}{}", col, ch, RESET);
                }
            }
        }
 
        // --- Shooting stars ---
        for star in &self.stars {
            let base_col = self.color_provider.get(t_abs, star.seed);
            let head_prefix = if star.depth > 0.75 { "" } else { DIM };
 
            for i in 0..star.length {
                let x = (star.x - star.dx * i as f64) as i32;
                let y = (star.y - star.dy * i as f64) as i32;
                if x >= 0 && x < w as i32 && y >= 0 && y < h as i32 {
                    let (col, ch): (String, &str) = if i == 0 {
                        (format!("{}{}", head_prefix, base_col), "@")
                    } else {
                        let trail_col = if star.depth > 0.7 {
                            "\x1b[38;5;240m".to_string()
                        } else {
                            "\x1b[38;5;238m".to_string()
                        };
                        let trail_ch = if i < star.length / 2 { "*" } else { "." };
                        (trail_col, trail_ch)
                    };
                    buf[y as usize][x as usize] = format!("{}{}{}", col, ch, RESET);
                }
            }
        }
 
        // --- Explosions ---
        // We need a local RNG here for the random explosion characters.
        let mut rng = rand::rng();
        let explosion_chars: &[u8] = b"*+x0123456789abcdef";
 
        for p in &self.explosions {
            let x = p.x as i32;
            let y = p.y as i32;
            if x >= 0 && x < w as i32 && y >= 0 && y < h as i32 {
                let base_col = self.color_provider.get(t_abs, p.seed);
                let col = if p.depth > 0.75 {
                    base_col
                } else {
                    format!("{}{}", DIM, base_col)
                };
                let ch_idx = rng.random_range(0..explosion_chars.len());
                let ch = explosion_chars[ch_idx] as char;
                buf[y as usize][x as usize] = format!("{}{}{}", col, ch, RESET);
            }
        }
 
        // Join buffer rows into the final frame string.
        // `map` + `collect` is Rust's functional-style equivalent of a list
        // comprehension. `join("\n")` works the same as Python's str.join.
        buf.iter()
            .map(|row| row.join(""))
            .collect::<Vec<_>>()
            .join("\n")
    }
}