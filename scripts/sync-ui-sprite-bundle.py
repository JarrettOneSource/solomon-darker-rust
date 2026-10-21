#!/usr/bin/env python3
"""Write stock sprite-only records from the Website's retained native UI manifest.

ControlPanel's retail bundle interleaves bitmap-font metrics with sprite headers.
The client only draws its sprite records, so reuse their already recovered frames,
logical boxes, registration and points in the existing sprite-only bundle codec.
"""

import json
from pathlib import Path
import struct
import sys

manifest_path, atlas_name, destination = sys.argv[1:]
manifest = json.loads(Path(manifest_path).read_text())
records = manifest["atlases"][atlas_name]["records"]
output = bytearray()
for index in range(len(records)):
    record = records[str(index)]
    if record["rotated"]:
        raise ValueError(f"{atlas_name}.{index} unexpectedly uses a rotated frame")
    x, y, width, height = record["frame"]
    logical_width, logical_height = record["logicalSize"]
    trim_x, trim_y = record["trimOrigin"]
    center_x = trim_x - (logical_width - width) / 2
    center_y = trim_y - (logical_height - height) / 2
    points = record["points"]
    output.extend(struct.pack(
        "<4fiI4fBI", x, y, width, height, logical_width, logical_height,
        width, height, center_x, center_y, 0, len(points),
    ))
    for point in points:
        output.extend(struct.pack("<2f", *point))
Path(destination).write_bytes(output)
