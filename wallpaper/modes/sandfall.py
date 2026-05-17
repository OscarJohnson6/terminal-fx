from ..base_mode import ModeBase
from ..color import ColorProvider
from ..ansi import RESET
from typing import List, Dict, Optional
import random


class SandMode(ModeBase):
    """
    Simple falling-sand / sand pile simulation.
    Grains spawn from the top and settle in a heap.
    """

    def __init__(self, speed_factor: float, color_provider: ColorProvider):
        self.speed_factor = speed_factor
        self.color_provider = color_provider

        self.last_dims: Optional[tuple[int, int]] = None
        self.grid: List[List[int]] = []   # grid[y][x] = grain index or -1
        self.grains: List[Dict] = []      # {x, y, seed}
        self.time_accum = 0.0

        # Simulation parameters
        self.step_dt = 1.0 / 60.0   # sand physics step
        self.spawn_rate = 0.4       # grains per column per second approx
        self.spawn_accum = 0.0

    def _init_world(self, width: int, height: int):
        self.last_dims = (width, height)
        self.grid = [[-1 for _ in range(width)] for _ in range(height)]
        self.grains.clear()
        self.time_accum = 0.0
        self.spawn_accum = 0.0

    def _spawn_grains(self, width: int, height: int, dt: float):
        # spawn some sand from the top row
        self.spawn_accum += width * self.spawn_rate * dt * max(0.3, self.speed_factor)
        num_to_spawn = int(self.spawn_accum)
        if num_to_spawn <= 0:
            return
        self.spawn_accum -= num_to_spawn

        top_y = 0
        for _ in range(num_to_spawn):
            x = random.randint(0, width - 1)
            if self.grid[top_y][x] != -1:
                continue  # column blocked at top

            idx = len(self.grains)
            seed = random.randint(0, 1000000)
            self.grains.append({"x": x, "y": top_y, "seed": seed})
            self.grid[top_y][x] = idx

    def _step_sand(self, width: int, height: int):
        if not self.grains:
            return

        # We will move grains and rebuild grid indexing.
        # Start with all cells empty, mark as we move.
        for y in range(height):
            for x in range(width):
                self.grid[y][x] = -1

        # randomize order to avoid directional bias
        order = list(range(len(self.grains)))
        random.shuffle(order)

        for idx in order:
            g = self.grains[idx]
            x = g["x"]
            y = g["y"]

            if y + 1 >= height:
                # bottom of screen
                self.grid[y][x] = idx
                continue

            # Try straight down
            if self.grid[y + 1][x] == -1:
                y += 1
            else:
                # try down-left or down-right, randomize side
                dirs = [-1, 1]
                random.shuffle(dirs)
                moved = False
                for dx in dirs:
                    nx = x + dx
                    ny = y + 1
                    if 0 <= nx < width and self.grid[ny][nx] == -1:
                        x, y = nx, ny
                        moved = True
                        break
                if not moved:
                    # can't move
                    pass

            g["x"] = x
            g["y"] = y
            self.grid[y][x] = idx

    def update(self, dt: float, width: int, height: int, t_abs: float):
        if width < 10 or height < 5:
            return

        dims = (width, height)
        if self.last_dims != dims or not self.grid:
            self._init_world(width, height)

        # spawn new grains
        self._spawn_grains(width, height, dt)

        # fixed-step sand physics
        self.time_accum += dt * max(0.5, self.speed_factor)
        while self.time_accum >= self.step_dt:
            self.time_accum -= self.step_dt
            self._step_sand(width, height)

    def render(self, width: int, height: int, t_abs: float) -> str:
        buf = [[" " for _ in range(width)] for _ in range(height)]

        for g in self.grains:
            x = g["x"]
            y = g["y"]
            if 0 <= x < width and 0 <= y < height:
                col = self.color_provider.get(t_abs, g["seed"])
                # small variety in glyphs
                ch = random.choice([".", ":", "*"])
                buf[y][x] = col + ch + RESET

        return "\n".join("".join(row) for row in buf)
