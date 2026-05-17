# TerminalFX  
### A Terminal-Based Wallpaper Animation Engine in Python

TerminalFX is a modular, high-performance animation engine that turns your terminal into a living wallpaper.  
It includes multiple visual modes (Matrix rain, fireworks, pendulum physics, particle bursts, waves, stars, orbs, and more) and supports custom color themes, spectrum cycling, animation speed control, and smooth rendering.

Supports Windows, macOS, and Linux — with full ANSI compatibility.

Upon adding a mode, add the class to init, main, and list in menu.

---

## 🚀 Features

- **High-FPS terminal animations** (smooth, flicker-minimized)
- **Modular mode system** — easily add new animation classes
- **Selectable modes** via interactive menu
- **Customizable color themes**
- **Spectrum & rainbow color providers**
- **Physics-driven animations** (pendulum, orbs, bounces)
- **Particle FX** (fireworks, stars, bursts)
- **Trail effects**  
- **Desktop launcher support** (BAT file, PowerShell function)

---

## 📦 Running the Program

From the parent directory of the `wallpaper` package, simply run:

```bash
python -m wallpaper


function pyWallpaper {
    cd "C:\Users\Oscar\MainDocument\Personal\Code\Personal\PythonScripts"
    python -m wallpaper
}

pyWallpaper
```
Or create a bat file for an easy click.

```bat
@echo off
cd /d "C:\Users\Oscar\MainDocument\Personal\Code\Personal\PythonScripts"
python -m wallpaper
```

## Directory
```text
wallpaper/
│
├── __main__.py              # Entry point for `python -m wallpaper`
├── main.py                  # Main engine: loop, rendering, dispatch, mode selection
├── ansi.py                  # ANSI codes (CLEAR, RESET, cursor hide/show, styles)
├── base_mode.py             # Abstract ModeBase class for all animation modes
├── color.py                 # ColorProvider system (themes, spectrum, rainbow)
├── menu.py                  # Text UI prompts: select mode, speed, color options
│
├── modes/                   # All available animation modes
│   ├── __init__.py          # Exports all Mode classes
│   ├── scroll.py            # Fake command-line scrolling log
│   ├── wave_full.py         # Full-screen sine-wave animation
│   ├── wave_line.py         # Snake/slither-style moving text line
│   ├── matrix.py            # Falling matrix digits
│   ├── fireworks.py         # Particle-based fireworks with trails & bursts
│   ├── shootingstars.py     # Shooting star animation with trails
│   ├── bursts.py            # Radial particle explosion bursts
│   ├── orbs.py              # Orbital motion / multi-object physics
│   ├── bounce.py            # Gravity + bounce simulation
│   ├── pendulum.py          # Double pendulum physics animation
│   ├── pulse.py             # Pulsing glowing center animation
│
└── pyproject.toml           # (Optional) For packaging / PEP 621 support
```
Copyright &copy; 2025       [Oscar Johnson](https://example.com/johndoe)
