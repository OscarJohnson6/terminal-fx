import time
import shutil
import sys

from .ansi import (
    CLEAR, HOME, HIDE_CURSOR, SHOW_CURSOR, RESET, FPS,
    enable_ansi_on_windows,
)

from .menu import (
    ask_mode,
    ask_speed_factor,
    ask_color_speed_factor,
    ask_color_mode,
)

from .modes import (
    ScrollMode, WaveFullMode, WaveLineMode, MatrixMode,
    FireworksMode, BurstsMode, ShootingStarMode, OrbsMode,
    BounceMode, PendulumMode, PulseMode, WormFieldMode,
    OrbitalsMode, SandMode,
    NebulaMode, FireMode, VillageWorldMode, MeteorShowerMode
)

MODE_CLASSES = {
    "scroll":    ScrollMode,
    "wave_full": WaveFullMode,
    "wave_line": WaveLineMode,
    "matrix":    MatrixMode,
    "fireworks": FireworksMode,
    "bursts":    BurstsMode,
    "stars":     ShootingStarMode,
    "orbs":      OrbsMode,
    "bounce":    BounceMode,
    "pendulum":  PendulumMode,
    "pulse":     PulseMode,
    "worm":      WormFieldMode,
    "orbitals":  OrbitalsMode,
    "sand":      SandMode,
    "nebula":    NebulaMode,
    "fire":      FireMode,
    "village":   VillageWorldMode,
    "meteors":    MeteorShowerMode,
}


def run_mode(mode):
    """
    Run a single mode until the user interrupts (Ctrl+C).
    Any exception inside the mode just returns to the menu,
    instead of crashing the whole program.
    """
    # Clear and hide cursor once when entering the mode
    sys.stdout.write(HIDE_CURSOR + CLEAR)
    sys.stdout.flush()

    last_time = time.perf_counter()
    start_time = last_time
    frame_time = 1.0 / FPS

    write = sys.stdout.write
    flush = sys.stdout.flush

    try:
        while True:
            now = time.perf_counter()
            dt = now - last_time
            last_time = now
            t_abs = now - start_time

            width, height = shutil.get_terminal_size((80, 24))

            # If terminal is too small, just skip the frame gracefully
            if width < 5 or height < 3:
                time.sleep(0.1)
                continue

            # Update and render current mode
            mode.update(dt, width, height, t_abs)
            frame = mode.render(width, height, t_abs)

            write(HOME)
            write(frame)
            write(RESET)
            flush()

            elapsed = time.perf_counter() - now
            sleep_for = frame_time - elapsed
            if sleep_for > 0:
                time.sleep(sleep_for)

    except KeyboardInterrupt:
        # Normal exit from a mode: user pressed Ctrl+C
        return
    except Exception as e:
        # Don't crash everything on a bug in a mode;
        # show a message and go back to the menu.
        write(RESET + SHOW_CURSOR + "\n")
        flush()
        print(f"\n[ERROR] Mode crashed: {e}")
        input("Press ENTER to return to the menu...")
        return


def main():
    enable_ansi_on_windows()

    try:
        while True:
            print("Fake Terminal Wallpaper\n")

            mode_name = ask_mode()
            # Optional: allow user to type 'q' in the menu to quit
            if mode_name is None:
                # If ask_mode returns None for "quit", bail out
                break
            if mode_name == "q":
                break

            speed_factor = ask_speed_factor()
            color_speed_factor = ask_color_speed_factor()
            color_provider = ask_color_mode(color_speed_factor)

            ModeClass = MODE_CLASSES.get(mode_name)
            if ModeClass is None:
                print(f"Unknown mode '{mode_name}'.")
                time.sleep(1.0)
                continue

            mode = ModeClass(speed_factor, color_provider)

            # Run the chosen mode until user interrupts
            run_mode(mode)

            # Always restore cursor before we ask for input
            sys.stdout.write(SHOW_CURSOR + RESET + "\n")
            sys.stdout.flush()

            choice = input(
                "Press ENTER for another mode, or 'q' to quit: "
            ).strip().lower()
            if choice == "q":
                break

    finally:
        # No matter what happens, make sure the cursor is visible again
        sys.stdout.write(SHOW_CURSOR + RESET + "\n")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
