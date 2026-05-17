# TerminalFX
### Terminal-Based Wallpaper Animation Engine

TerminalFX is a modular animation engine that renders dynamic visuals within the command line. Originally developed in Python, the project is currently being ported to Rust to improve rendering frequency, memory safety, and computational performance for complex physics simulations.

---

## Project Structure

This repository uses a monorepo structure to house both implementations of the engine:

| Directory | Language | Status | Focus |
| :--- | :--- | :--- | :--- |
| [**`wallpaper/`**](./wallpaper) | Python | Stable | Rapid prototyping, 15+ animation modes. |
| [**`terminal_wallpaper/`**](./terminal_wallpaper) | Rust | Active Dev | Low-level optimization, memory safety. |

---

## Core Features

* **Dynamic Visuals:** Matrix rain, double-pendulums, particle explosions, and orbital mechanics.
* **ANSI Compatibility:** Full support for Windows, macOS, and Linux terminals.
* **Modular Architecture:** Standardized API for implementing custom animation classes.
* **Physics Engine:** Integrated logic for gravity, collisions, and fluid-like motion.

---

## Execution

### Python Implementation
Requires Python 3.10 or higher.
```powershell
cd wallpaper
python -m wallpaper