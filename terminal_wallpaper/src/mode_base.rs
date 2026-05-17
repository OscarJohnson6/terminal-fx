// ===================================================================
//  src/mode_base.rs
// -------------------------------------------------------------------
//  Core animation trait that every mode must implement.
//
//  ARCHITECTURE
//  Modes are pure simulation + rendering units. They have no
//  knowledge of the terminal, raw mode, input, or frame pacing —
//  all of that lives in `main.rs::run_mode_live`. Modes only:
//    1. Advance their simulation state (update)
//    2. Produce a rendered terminal string (render)
//
//  The decoupling means modes are straightforward to unit-test and
//  simple to add: implement two functions, register in mode_registry.
//
//  TRAIT OBJECTS VS GENERICS
//  `Box<dyn Mode>` is used at the call site so modes can be selected
//  at runtime from user input. This incurs a single vtable dispatch
//  per frame — negligible compared to the render work itself.
//  If a future mode needs static dispatch for performance, it can
//  be called directly without going through this trait.
//
//  IMPLEMENTING A NEW MODE
//  pub struct MyMode { ... }
//  impl Mode for MyMode {
//      fn update(&mut self, dt: f64, width: u16, height: u16, t_abs: f64) {
//          // Advance physics / simulation by dt seconds.
//          // Store any state you need in render() on self.
//      }
//      fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
//          // Build the complete frame string. Every cell should be
//          // written — do not assume any terminal state from last frame.
//          // End each row with '\n'. The caller writes HOME + frame.
//      }
//  }
// ===================================================================

/// The shared interface for all animation modes.
///
/// `update` and `render` are called once per frame in that order.
/// The calling code in `run_mode_live` handles timing, terminal I/O,
/// and input; modes only need to concern themselves with simulation
/// state and producing their output string.
pub trait Mode {
    /// Advance the simulation by `dt` seconds.
    ///
    /// - `dt` is the wall-clock seconds elapsed since the previous
    ///   frame. Multiply velocities and rates by `dt` to get
    ///   frame-rate-independent motion.
    /// - `t_abs` is the total seconds since the mode started. Use
    ///   this for oscillators and cyclic effects instead of
    ///   accumulating your own timer — it avoids float drift.
    /// - `width` and `height` are the current terminal dimensions in
    ///   columns and rows. Update any layout-dependent state here
    ///   when these change rather than caching them between frames.
    fn update(&mut self, dt: f64, width: u16, height: u16, t_abs: f64);

    /// Produce the complete terminal frame as a `String`.
    ///
    /// The returned string must cover every cell of a `width × height`
    /// grid with rows separated by `'\n'`. The caller writes the HOME
    /// escape before the string and RESET after, so the mode does not
    /// need to handle cursor positioning or final cleanup.
    ///
    /// `render` takes `&self` (shared / immutable borrow) so it cannot
    /// modify simulation state. Put any state that must change during
    /// rendering (e.g. per-render RNG) inside `update` instead, or
    /// use interior mutability (`Cell`, `RefCell`) with clear
    /// documentation explaining why.
    fn render(&self, width: u16, height: u16, t_abs: f64) -> String;
}