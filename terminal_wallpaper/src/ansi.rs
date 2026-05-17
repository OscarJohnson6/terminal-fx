// ===================================================================
//  src/ansi.rs
// -------------------------------------------------------------------
//  Low-level terminal control: ANSI escape sequences, colour helpers,
//  frame-rate utilities, and Windows VT enable.
//
//  Design notes:
//   - All escape sequences are `&'static str` constants so they can be
//     inlined wherever needed without allocation.
//   - Colour helpers return owned `String` because they're constructed
//     per-pixel during render — the formatter has to allocate anyway.
//   - DEC Synchronized Output (mode 2026) is opt-in: emitting it is
//     harmless on terminals that don't understand it.
// ===================================================================

// ── Cursor / screen control ─────────────────────────────────────────

/// Move cursor to row 1, column 1 (top-left).
pub const HOME: &str = "\x1b[H";

/// Reset all SGR attributes (colour, bold, dim, etc.) to defaults.
pub const RESET: &str = "\x1b[0m";

/// SGR "dim" / decreased intensity. Some terminals render this as a
/// faint version of the current foreground colour.
pub const DIM: &str = "\x1b[2m";

// ── Synchronized Output (DEC private mode 2026) ────────────────────
//
// When a terminal supports this mode, it suspends its display refresh
// between SYNC_BEGIN and SYNC_END, then commits all queued writes as a
// single atomic update. This eliminates flicker caused by partial-frame
// repaints — the user sees only complete frames, never an animation
// underneath without its overlay.
//
// Supported by Windows Terminal, WezTerm, Alacritty (≥0.13), foot,
// kitty, mintty, iTerm2, and most modern TUIs. Older terminals receive
// these as unknown private-mode escapes and silently ignore them, which
// makes them safe to emit unconditionally.

/// Begin a synchronized update batch.
pub const SYNC_BEGIN: &str = "\x1b[?2026h";

/// End a synchronized update batch (commits all buffered writes).
pub const SYNC_END: &str = "\x1b[?2026l";

// ── Frame-rate utilities ────────────────────────────────────────────

/// Default target frame rate for modes that don't specify their own.
pub const FPS: u32 = 60;

/// Frame duration for the default FPS.
pub fn frame_duration() -> std::time::Duration {
    frame_duration_for_fps(FPS)
}

/// Frame duration for an explicit FPS value, clamped to [1, 240]
/// to avoid divide-by-zero and absurd target rates.
pub fn frame_duration_for_fps(fps: u32) -> std::time::Duration {
    let fps = fps.clamp(1, 240);
    std::time::Duration::from_secs_f64(1.0 / fps as f64)
}

// ── True-colour helpers ─────────────────────────────────────────────

/// Build a 24-bit foreground SGR sequence: `\x1b[38;2;r;g;bm`.
pub fn rgb(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[38;2;{};{};{}m", r, g, b)
}

/// Build a 24-bit background SGR sequence: `\x1b[48;2;r;g;bm`.
pub fn bg_rgb(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[48;2;{};{};{}m", r, g, b)
}

// ── Platform setup ──────────────────────────────────────────────────

/// On Windows, calling into crossterm's terminal mode initialiser
/// causes `ENABLE_VIRTUAL_TERMINAL_PROCESSING` to be set on the console
/// handle, which is what makes ANSI sequences work in cmd / PowerShell.
/// On other platforms this is a no-op.
#[cfg(windows)]
pub fn enable_ansi_on_windows() {
    let _ = crossterm::terminal::enable_raw_mode();
}

#[cfg(not(windows))]
pub fn enable_ansi_on_windows() {}