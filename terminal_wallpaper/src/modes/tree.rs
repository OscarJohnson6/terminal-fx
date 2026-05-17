// ===== src/modes/tree.rs =====
//
// Life cycle:   Seed (2s) → Growing (branches animate out one by one)
//             → Mature (6s, tips sway and sparkle)
//             → Seed falls from canopy, lands, cycle restarts there.
//
// CORE TRICK: Pre-generate all branch segments at init with a `start_t`
// per branch (when it begins growing) and a `duration` (seconds to extend).
// In render we compute `progress = (anim_t - start_t) / duration` per branch
// and draw only up to `lerp(base, tip, progress)`. The tree "grows" by
// branches becoming progressively visible from root outward.
//
// BORROW TRICK: We can't call `self.init()` while match-borrowing `self.stage`,
// so we compute an `Action` enum value inside the borrow, drop it, then act.
 
use crate::ansi::{rgb, RESET};
use crate::color::ColorProvider;
use crate::mode_base::Mode;
use rand::RngExt;
use std::f64::consts::FRAC_PI_2;
 
// ── Branch segment ────────────────────────────────────────────────────────────
 
struct Seg {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    depth: u32,
    start_t: f64,   // anim_t (seconds) when this branch begins growing
    duration: f64,  // seconds to fully extend
}
 
// ── Stage machine ─────────────────────────────────────────────────────────────
 
enum Stage {
    Seed    { timer: f64 },
    Growing { anim_t: f64 },
    Mature  { timer: f64 },
    Falling { seed_x: f64, seed_y: f64, vy: f64 },
}
 
// ── Mode struct ───────────────────────────────────────────────────────────────
 
pub struct TreeMode {
    color_provider: ColorProvider,
    speed: f64,
    stage: Stage,
    segs: Vec<Seg>,
    base_x: f64,
    base_y: f64,
    total_anim_t: f64,  // when the last branch finishes growing
    tip_x: f64,         // highest branch tip — seed drop point
    tip_y: f64,
    initialized: bool,
}
 
impl TreeMode {
    pub fn new(speed: f64, color_provider: ColorProvider) -> Self {
        Self {
            color_provider,
            speed,
            stage: Stage::Seed { timer: 0.0 },
            segs: Vec::new(),
            base_x: 40.0,
            base_y: 22.0,
            total_anim_t: 10.0,
            tip_x: 40.0,
            tip_y: 5.0,
            initialized: false,
        }
    }
 
    /// Build a fresh random tree at (`bx`, `by`).
    fn init_tree(&mut self, bx: f64, by: f64, width: u16, height: u16) {
        let mut rng = rand::rng();
        self.base_x = bx;
        self.base_y = by;
        self.segs.clear();
 
        let trunk_len = height as f64 * rng.random_range(0.22..0.32);
        let lean      = rng.random_range(-0.18_f64..0.18_f64);
 
        gen_segs(
            &mut self.segs,
            bx, by,
            -FRAC_PI_2 + lean,
            trunk_len,
            0,
            0.0,  // root: parent starts at t=0
            0.0,  // root: parent duration doesn't matter (start_t forced to 0)
            &mut rng,
        );
 
        // When does the last branch finish?
        self.total_anim_t = self.segs.iter()
            .map(|s| s.start_t + s.duration)
            .fold(0.0_f64, f64::max);
 
        // Choose the highest visible branch tip as the seed drop point.
        // In terminal coords, lower y = higher on screen.
        let margin = 1.5_f64;
        if let Some(s) = self.segs.iter()
            .filter(|s| s.depth >= 5
                && s.x2 >= margin && s.x2 < width as f64 - margin
                && s.y2 >= margin)
            .min_by(|a, b| a.y2.partial_cmp(&b.y2).unwrap_or(std::cmp::Ordering::Equal))
        {
            self.tip_x = s.x2;
            self.tip_y = s.y2;
        } else {
            self.tip_x = bx;
            self.tip_y = by - 4.0;
        }
 
        self.initialized = true;
    }
}
 
// ── Recursive segment generator ───────────────────────────────────────────────
//
// Children start growing when their parent is ~65% done, creating a cascade
// from trunk to leaf tips. `parent_start_t` + `parent_dur` let us compute
// when the parent is 65% done.
 
fn gen_segs(
    segs: &mut Vec<Seg>,
    x: f64, y: f64,
    angle: f64, len: f64,
    depth: u32,
    parent_start_t: f64, parent_dur: f64,
    rng: &mut impl RngExt,
) {
    if depth > 9 || len < 1.1 { return; }
 
    // Root starts at t=0; every subsequent branch starts at parent's 65%-done mark.
    let start_t = if depth == 0 {
        0.0
    } else {
        parent_start_t + parent_dur * 0.65
    };
 
    // Deeper branches grow faster (shorter duration relative to length).
    let speed_scale = 1.0 + depth as f64 * 0.07;
    let duration    = (len / (9.0 * speed_scale)).max(0.12);
 
    let ex = x + angle.cos() * len;
    let ey = y + angle.sin() * len;
 
    segs.push(Seg { x1: x, y1: y, x2: ex, y2: ey, depth, start_t, duration });
 
    let n: usize = match depth {
        0 | 1 => 2,
        2 | 3 => rng.random_range(2usize..=3),
        _     => rng.random_range(1usize..=2),
    };
    let spread = rng.random_range(0.38_f64..0.72_f64);
 
    for i in 0..n {
        let frac      = if n == 1 { 0.5 } else { i as f64 / (n - 1) as f64 };
        let angle_off = (frac - 0.5) * 2.0 * spread + rng.random_range(-0.07..0.07);
        let child_len = len * rng.random_range(0.62..0.78);
        gen_segs(segs, ex, ey, angle + angle_off, child_len, depth + 1, start_t, duration, rng);
    }
}
 
// ── Helpers ───────────────────────────────────────────────────────────────────
 
fn lerp_ch(a: u8, b: u8, t: f64) -> u8 {
    (a as f64 + (b as f64 - a as f64) * t.clamp(0.0, 1.0)) as u8
}
 
/// Dark trunk-brown at depth 0, bright leaf-green at depth 8+.
fn branch_color(depth: u32) -> (u8, u8, u8) {
    let t = (depth as f64 / 8.0).clamp(0.0, 1.0);
    (lerp_ch(80, 28, t), lerp_ch(46, 165, t), lerp_ch(15, 22, t))
}
 
/// Best single character for a line whose overall direction is (dx, dy).
fn seg_char(dx: f64, dy: f64, depth: u32) -> char {
    if depth >= 7 { return '*'; }
    if depth >= 5 { return '\u{00B7}'; } // ·
    let (adx, ady) = (dx.abs(), dy.abs());
    if ady > adx * 2.0       { '│' }
    else if adx > ady * 2.0  { '─' }
    else if (dx > 0.0) == (dy < 0.0) { '/' }  // up-right or down-left
    else                     { '\\' }
}
 
/// Draw a straight line from (x1,y1) to (x2,y2), placing `ch` at each cell.
fn draw_seg(buf: &mut Vec<Vec<String>>, w: usize, h: usize,
            x1: f64, y1: f64, x2: f64, y2: f64, col: &str, ch: char) {
    let dx    = x2 - x1;
    let dy    = y2 - y1;
    let steps = (dx.abs().max(dy.abs()) as usize + 1).max(1);
    for i in 0..=steps {
        let t  = i as f64 / steps as f64;
        let px = (x1 + dx * t).round() as i32;
        let py = (y1 + dy * t).round() as i32;
        if px >= 0 && px < w as i32 && py >= 0 && py < h as i32 {
            buf[py as usize][px as usize] = format!("{}{}{}", col, ch, RESET);
        }
    }
}
 
// ── Mode impl ─────────────────────────────────────────────────────────────────
 
impl Mode for TreeMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        let dt = dt * self.speed;
        let ground_y = height as f64 - 2.0;
 
        if !self.initialized {
            let bx = width as f64 * 0.5;
            self.init_tree(bx, ground_y, width, height);
            return;
        }
 
        // ── Compute stage transitions without holding a borrow into self ──
        enum Act { None, StartGrow, GoMature, Drop, Restart(f64, f64) }
 
        let act = match &mut self.stage {
            Stage::Seed { timer } => {
                *timer += dt;
                if *timer >= 2.2 { Act::StartGrow } else { Act::None }
            }
            Stage::Growing { anim_t } => {
                *anim_t += dt;
                if *anim_t >= self.total_anim_t + 1.2 { Act::GoMature } else { Act::None }
            }
            Stage::Mature { timer } => {
                *timer += dt;
                if *timer >= 6.0 { Act::Drop } else { Act::None }
            }
            Stage::Falling { seed_x, seed_y, vy } => {
                *vy      += 6.0 * dt;   // gravity
                *seed_y  += *vy * dt;
                if *seed_y >= ground_y {
                    Act::Restart(*seed_x, ground_y)
                } else {
                    Act::None
                }
            }
        };
 
        // ── Apply transitions ─────────────────────────────────────────────
        match act {
            Act::StartGrow => {
                // Generate a new random tree at the current base position,
                // then switch to the growing stage.
                let bx = self.base_x;
                let by = self.base_y;
                self.init_tree(bx, by, width, height);
                self.stage = Stage::Growing { anim_t: 0.0 };
            }
            Act::GoMature => {
                self.stage = Stage::Mature { timer: 0.0 };
            }
            Act::Drop => {
                let (tx, ty) = (self.tip_x, self.tip_y);
                self.stage = Stage::Falling { seed_x: tx, seed_y: ty, vy: 1.5 };
            }
            Act::Restart(nx, ny) => {
                // New tree grows where the seed landed — gives a nice continuity.
                let nx = nx.clamp(4.0, width as f64 - 4.0);
                self.init_tree(nx, ny, width, height);
                self.stage = Stage::Seed { timer: 0.0 };
            }
            Act::None => {}
        }
    }
 
    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w = width  as usize;
        let h = height as usize;
        let mut buf = vec![vec![" ".to_string(); w]; h];
 
        // ── Ground ────────────────────────────────────────────────────────
        let ground_row = (self.base_y as usize).min(h.saturating_sub(1));
        for y in ground_row..h {
            let (r, g, b) = if y == ground_row {
                (54, 90, 28)   // surface grass
            } else {
                (76, 53, 28)   // dirt below
            };
            for x in 0..w {
                buf[y][x] = format!("{}▓{}", rgb(r, g, b), RESET);
            }
        }
 
        // ── Seed (dot on the ground before growing starts) ────────────────
        if let Stage::Seed { .. } = &self.stage {
            let sx = self.base_x as usize;
            let sy = ground_row;
            if sx < w && sy < h {
                buf[sy][sx] = format!("{}◉{}", rgb(140, 92, 44), RESET);
            }
            // Nothing else to draw yet.
            return buf.into_iter().map(|r| r.join("")).collect::<Vec<_>>().join("\n");
        }
 
        // ── Current animation time for branch growth ──────────────────────
        let anim_t = match &self.stage {
            Stage::Growing { anim_t } => *anim_t,
            _                        => self.total_anim_t + 99.0, // fully grown
        };
 
        // ── Sway timer (only Mature stage) ────────────────────────────────
        let sway_t = match &self.stage {
            Stage::Mature { timer } => *timer,
            _ => 0.0,
        };
 
        // ── Draw each branch up to its current growth progress ────────────
        for seg in &self.segs {
            if anim_t < seg.start_t { continue; }
            let progress = ((anim_t - seg.start_t) / seg.duration).clamp(0.0, 1.0);
            if progress < 0.01 { continue; }
 
            // Interpolate tip from base toward full endpoint.
            let mut cur_x2 = seg.x1 + (seg.x2 - seg.x1) * progress;
            let cur_y2 = seg.y1 + (seg.y2 - seg.y1) * progress;
 
            // In Mature stage, deep branches sway in the breeze.
            if seg.depth >= 4 && sway_t > 0.0 {
                let sway_amp = (seg.depth as f64 - 3.0) * 0.55;
                let sway_off = (sway_t * 1.9 + seg.x1 * 0.13).sin() * sway_amp;
                cur_x2 += sway_off;
            }
 
            let dx = seg.x2 - seg.x1;
            let dy = seg.y2 - seg.y1;
            let (r, g, b) = branch_color(seg.depth);
            let col = rgb(r, g, b);
            let ch  = seg_char(dx, dy, seg.depth);
 
            draw_seg(&mut buf, w, h, seg.x1, seg.y1, cur_x2, cur_y2, &col, ch);
        }
 
        // ── Leaf sparkle in Mature stage ──────────────────────────────────
        if let Stage::Mature { .. } = &self.stage {
            let leaf_chars = ['✿', '*', '\u{00B7}', '\u{25E6}', '✿', '\u{00B7}'];
            for seg in self.segs.iter().filter(|s| s.depth >= 7) {
                let lx = seg.x2 as i32;
                let ly = seg.y2 as i32;
                if lx >= 0 && lx < w as i32 && ly >= 0 && ly < h as i32 {
                    // Each tip cycles through chars at a slightly different rate.
                    let idx = ((t_abs * 1.6 + seg.x1 * 0.41 + seg.y1 * 0.17) as usize)
                               % leaf_chars.len();
                    buf[ly as usize][lx as usize] =
                        format!("{}{}{}",  rgb(30, 210, 50), leaf_chars[idx], RESET);
                }
            }
        }
 
        // ── Falling seed ──────────────────────────────────────────────────
        if let Stage::Falling { seed_x, seed_y, .. } = &self.stage {
            // Draw a little arc streak trailing the seed for motion feel.
            for streak in 0..3 {
                let sy = (*seed_y as i32) - streak;
                let sx = *seed_x as i32;
                if sx >= 0 && sx < w as i32 && sy >= 0 && sy < h as i32 {
                    let (sr, sg, sb) = if streak == 0 { (170, 110, 45) } else { (90, 60, 25) };
                    let ch = if streak == 0 { '◉' } else { '\u{00B7}' };
                    buf[sy as usize][sx as usize] = format!("{}{}{}", rgb(sr, sg, sb), ch, RESET);
                }
            }
        }
 
        // ── Color-vibe tint overlay on the leaves ─────────────────────────
        // `color_provider.get()` returns an ANSI string, but we already have
        // pixel colors baked in. Use it for the trunk's shadow tinting instead.
        let _ = self.color_provider.get(t_abs, self.base_x as i32);
 
        buf.into_iter().map(|r| r.join("")).collect::<Vec<_>>().join("\n")
    }
}