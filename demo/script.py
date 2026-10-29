# demo: standard library only
from dataclasses import dataclass


@dataclass
class App:
    name: str
    slots: int = 5
    enabled: bool = True


def describe(app):
    """one line summary"""
    return f"{app.name}: {app.slots} slots"


def total(apps):
    # a long line for the horizontal scrollbar: sum the slots of every enabled application in the list
    return sum(a.slots for a in apps if a.enabled)


APPS = [
    App("clock", 1),
    App("calculator", 1),
    App("calendar", 1),
    App("files", 1),
    App("preview", 1),
]


def main():
    for app in APPS:
        print(describe(app))
    print(f"total: {total(APPS)}")


if __name__ == "__main__":
    main()
