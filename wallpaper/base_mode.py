class ModeBase:
    def update(self, dt: float, width: int, height: int, t_abs: float):
        raise NotImplementedError

    def render(self, width: int, height: int, t_abs: float) -> str:
        raise NotImplementedError
