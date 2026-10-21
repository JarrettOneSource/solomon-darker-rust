#!/usr/bin/env python3
"""Reuse the Website's stock packed first-pose wizard layers for Hall/ally portraits."""
import json
import os
from pathlib import Path
import re
import shutil

root = Path(__file__).resolve().parent.parent
website = Path(os.environ.get("SOLOMON_WEBSITE_DIR", root.parent / "website-rust-current"))
source = website / "frontend/src/game/renderer/player-character-atlas.generated.ts"
lines = source.read_text().splitlines()


def table(name):
    line = next(line for line in lines if line.startswith(f"export const {name}:"))
    return json.loads(line.split(" = ", 1)[1])


rectangles = table("PLAYER_CHARACTER_ATLAS_RECTANGLES")
sheets = table("PLAYER_CHARACTER_ATLAS_SHEETS")
names = ["player-character-staff-back", "player-character-staff-front"]
for element in ("air", "earth", "ether", "fire", "water"):
    for layer in ("head", "robe-dynamic", "robe-fixed"):
        names.append(f"player-character-{layer}-{element}")

frames = {}
pages = set()
for name in names:
    columns, rows, _, _, packed = sheets[name]
    frames[name] = []
    for heading in range(rows):
        frame = packed[heading * columns]
        if frame is None:
            frames[name].append(None)
            continue
        rectangle, trim_x, trim_y = frame
        page, x, y, width, height = rectangles[rectangle]
        pages.add(page)
        frames[name].append([page, x, y, width, height, trim_x, trim_y])

# These are authored view tables, not a second weapon/emitter implementation. Keep the
# fixed preview pose with its packed art; live gameplay uses the simulation's socket owner.
weapon = (website / "frontend/src/game/core-kernels/native-player-weapon.ts").read_text()
pose_zero = re.search(r"STAFF_PRIMARY_EMITTER_OFFSETS.*?0: \[(.*?)\n  \]", weapon, re.S).group(1)
staff_orbs = [[float(x), float(y)] for x, y in re.findall(r"x: ([-\d.]+), y: ([-\d.]+)", pose_zero)]
presentation = (website / "frontend/src/game/player-character-presentation.ts").read_text()
front_bank = re.search(r"const STAFF_FRONT: readonly boolean\[\] = \[(.*?)\]", presentation, re.S).group(1)
staff_front = [value == "true" for value in re.findall(r"true|false", front_bank)]
if len(staff_orbs) != 24 or len(staff_front) != 24:
    raise ValueError("stock portrait staff tables must have 24 facings")

out = root / "web/assets/stock"
out.mkdir(parents=True, exist_ok=True)
for page in pages:
    name = f"player-character-atlas-{page}.png"
    shutil.copy2(website / "frontend/src/assets/game" / name, out / name)
(out / "player-portraits.json").write_text(json.dumps({
    "frames": frames, "staff_orbs": staff_orbs, "staff_front": staff_front,
}, separators=(",", ":")) + "\n")
