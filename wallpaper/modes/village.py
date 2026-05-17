from ..base_mode import ModeBase
from ..color import ColorProvider
from ..ansi import RESET
from typing import List, Optional
import random


class VillageWorldMode(ModeBase):
    """
    Tiny WorldBox-like sim:
    Two villages expand over the map and clash at the frontier.
    Completely automatic; just watch them grow and fight.
    """

    def __init__(self, speed_factor: float, color_provider: ColorProvider):
        self.speed_factor = speed_factor
        self.color_provider = color_provider

        self.last_dims: Optional[tuple[int, int]] = None

        self.owner: List[List[int]] = []  # 0 = empty, 1 = village A, 2 = village B
        self.age: List[List[int]] = []    # age of the tile
        self.pop: List[List[float]] = []  # 'population' density

        self.time_accum = 0.0
        self.step_dt = 1.0 / 15.0  # world step

    def _init_world(self, width: int, height: int):
        self.last_dims = (width, height)
        self.owner = [[0 for _ in range(width)] for _ in range(height)]
        self.age   = [[0 for _ in range(width)] for _ in range(height)]
        self.pop   = [[0.0 for _ in range(width)] for _ in range(height)]
        self.time_accum = 0.0

        if width < 10 or height < 5:
            return

        # Seed two villages: left and right
        y_center = height // 2
        x_a = width // 4
        x_b = (3 * width) // 4

        self.owner[y_center][x_a] = 1
        self.pop[y_center][x_a] = 6.0

        self.owner[y_center][x_b] = 2
        self.pop[y_center][x_b] = 6.0

    def _neighbors(self, x: int, y: int, width: int, height: int):
        # 4-neighborhood
        if x > 0:
            yield (x - 1, y)
        if x + 1 < width:
            yield (x + 1, y)
        if y > 0:
            yield (x, y - 1)
        if y + 1 < height:
            yield (x, y + 1)

    def _step_world(self, width: int, height: int):
        # randomize iteration order a bit to avoid directional bias
        coords = [(x, y) for y in range(height) for x in range(width)]
        random.shuffle(coords)

        # parameters
        max_pop = 10.0
        base_growth = 0.3
        expand_chance_base = 0.08
        attack_chance_base = 0.04

        for x, y in coords:
            own = self.owner[y][x]
            if own == 0:
                continue

            # Age + population growth
            self.age[y][x] = min(self.age[y][x] + 1, 1000000)
            p = self.pop[y][x]
            # simple capped growth
            if p < max_pop:
                p += base_growth * (1.0 - p / max_pop)
            self.pop[y][x] = p

            # more pop -> more likely to expand/attack
            expand_chance = expand_chance_base * (p / max_pop)
            attack_chance = attack_chance_base * (p / max_pop)

            # check neighbors
            empties = []
            enemies = []
            for nx, ny in self._neighbors(x, y, width, height):
                o2 = self.owner[ny][nx]
                if o2 == 0:
                    empties.append((nx, ny))
                elif o2 != own:
                    enemies.append((nx, ny))

            # expand into empty land
            if empties and random.random() < expand_chance:
                nx, ny = random.choice(empties)
                self.owner[ny][nx] = own
                self.pop[ny][nx] = max(1.0, p * 0.4)
                self.age[ny][nx] = 0

            # attack enemy tile
            if enemies and random.random() < attack_chance:
                nx, ny = random.choice(enemies)
                enemy_owner = self.owner[ny][nx]
                enemy_pop = self.pop[ny][nx]

                # crude power comparison: pop * random factor
                my_power = p * (0.6 + random.random())
                enemy_power = enemy_pop * (0.6 + random.random())

                if my_power > enemy_power:
                    # conquer tile
                    self.owner[ny][nx] = own
                    self.pop[ny][nx] = max(1.0, (my_power - enemy_power) * 0.3)
                    self.age[ny][nx] = 0
                    # attacker spends some population
                    self.pop[y][x] = max(0.5, p * 0.8)
                else:
                    # defender holds, attacker loses some pop
                    self.pop[y][x] = max(0.3, p * 0.7)

    def update(self, dt: float, width: int, height: int, t_abs: float):
        if width < 10 or height < 5:
            return

        dims = (width, height)
        if self.last_dims != dims or not self.owner:
            self._init_world(width, height)

        self.time_accum += dt * max(0.5, self.speed_factor)
        while self.time_accum >= self.step_dt:
            self.time_accum -= self.step_dt
            self._step_world(width, height)

    def render(self, width: int, height: int, t_abs: float) -> str:
        buf = [[" " for _ in range(width)] for _ in range(height)]

        for y in range(height):
            for x in range(width):
                own = self.owner[y][x]
                if own == 0:
                    # empty land: occasionally dot it so it’s not just blank
                    if random.random() < 0.02:
                        buf[y][x] = "."
                    continue

                p = self.pop[y][x]
                a = self.age[y][x]

                # choose glyph by pop/age
                if p < 2:
                    ch = "."
                elif p < 4:
                    ch = "·"
                elif p < 7:
                    ch = "o"
                elif p < 9:
                    ch = "O"
                else:
                    ch = "█"

                # different color seeds per village
                if own == 1:
                    seed = 1000 + x * 31 + y * 17
                else:
                    seed = 2000 + x * 29 + y * 19

                col = self.color_provider.get(t_abs * 0.4, seed)
                buf[y][x] = col + ch + RESET

        return "\n".join("".join(row) for row in buf)
