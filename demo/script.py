# demo: standard library only
from dataclasses import dataclass


@dataclass
class App:
    name: str
    slots: int = 5


def describe(app):
    """one line summary"""
    return f"{app.name}: {app.slots} slots"


if __name__ == "__main__":
    print(describe(App("wasm-fb")))
