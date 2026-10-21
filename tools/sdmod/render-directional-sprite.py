import argparse
import json
import math
import sys
import tempfile
from array import array
from pathlib import Path

import bpy
from mathutils import Vector


def arguments():
    values = sys.argv[sys.argv.index("--") + 1 :]
    parser = argparse.ArgumentParser()
    parser.add_argument("--recipe", required=True)
    parser.add_argument("--source-root", required=True)
    parser.add_argument("--output", required=True)
    return parser.parse_args(values)


def inside(root, relative):
    path = (root / relative).resolve()
    if path != root and root not in path.parents:
        raise ValueError(f"asset path leaves its source root: {relative}")
    if not path.is_file():
        raise ValueError(f"asset file is missing: {relative}")
    return path


def number(value, minimum, maximum, label):
    if not isinstance(value, (int, float)) or value < minimum or value > maximum:
        raise ValueError(f"{label} must be within {minimum}..{maximum}")
    return float(value)


def load_recipe(path):
    value = json.loads(path.read_text(encoding="utf-8"))
    required = {"animations", "blend", "camera", "frameSize", "headings", "output", "source"}
    if set(value) != required:
        raise ValueError("sprite recipe fields do not match the documented contract")
    frame_size = int(number(value["frameSize"], 32, 512, "frameSize"))
    headings = int(number(value["headings"], 1, 24, "headings"))
    if not isinstance(value["animations"], list) or not value["animations"]:
        raise ValueError("animations must be a nonempty array")
    rows = sum(len(animation.get("frames", [])) for animation in value["animations"])
    if rows < 1 or frame_size * headings > 4096 or frame_size * rows > 4096:
        raise ValueError("sprite sheet exceeds the 4096 px Web Lua asset limit")
    return value, frame_size, headings, rows


def load_blend_model(path):
    bpy.ops.wm.open_mainfile(filepath=str(path))
    armatures = [value for value in bpy.context.scene.objects if value.type == "ARMATURE"]
    meshes = [value for value in bpy.context.scene.objects if value.type == "MESH" and any(
        modifier.type == "ARMATURE" for modifier in value.modifiers
    )]
    if len(armatures) != 1 or len(meshes) != 1:
        raise ValueError("Blend source must contain one rigged mesh and one armature")
    keep = {armatures[0], meshes[0]}
    for value in list(bpy.context.scene.objects):
        if value not in keep:
            bpy.data.objects.remove(value, do_unlink=True)

    for material in meshes[0].data.materials:
        color = tuple(material.diffuse_color)
        material.use_nodes = True
        material.node_tree.nodes.clear()
        shader = material.node_tree.nodes.new("ShaderNodeBsdfPrincipled")
        output = material.node_tree.nodes.new("ShaderNodeOutputMaterial")
        material.node_tree.links.new(shader.outputs["BSDF"], output.inputs["Surface"])
        shader.inputs["Base Color"].default_value = color
        shader.inputs["Alpha"].default_value = 1
        shader.inputs["Roughness"].default_value = 0.82
        material.diffuse_color[3] = 1
    return armatures[0], meshes[0]


def find_action(name):
    matches = [value for value in bpy.data.actions if value.name.lower() == name.lower()]
    if len(matches) != 1:
        raise ValueError(f"animation action {name!r} was not found")
    return matches[0]


def point_at(camera, target):
    camera.rotation_euler = (Vector(target) - camera.location).to_track_quat("-Z", "Y").to_euler()


def configure_scene(frame_size, headings, camera_spec):
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_EEVEE_NEXT"
    scene.render.film_transparent = True
    scene.render.resolution_x = frame_size * headings
    scene.render.resolution_y = frame_size
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.image_settings.color_depth = "8"
    scene.render.image_settings.compression = 100
    looks = {value.identifier for value in scene.view_settings.bl_rna.properties["look"].enum_items}
    scene.view_settings.look = (
        "AgX - Medium High Contrast"
        if "AgX - Medium High Contrast" in looks
        else "Medium High Contrast"
    )

    elevation = math.radians(number(camera_spec.get("elevation", 58), 10, 85, "camera.elevation"))
    azimuth = math.radians(number(camera_spec.get("azimuthOffset", 0), -360, 360, "camera.azimuthOffset"))
    scale = number(camera_spec.get("orthographicScale", 4.4), 0.5, 50, "camera.orthographicScale")
    distance = scale * 2
    target = Vector((0, 0, scale * 0.42))
    camera_data = bpy.data.cameras.new("Sprite camera")
    camera_data.type = "ORTHO"
    camera_data.ortho_scale = scale * headings
    camera = bpy.data.objects.new("Sprite camera", camera_data)
    scene.collection.objects.link(camera)
    camera.location = (
        math.sin(azimuth) * math.cos(elevation) * distance,
        -math.cos(azimuth) * math.cos(elevation) * distance,
        target.z + math.sin(elevation) * distance,
    )
    point_at(camera, target)
    scene.camera = camera

    world = bpy.data.worlds.new("Sprite world")
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs["Color"].default_value = (0.04, 0.05, 0.08, 1)
    world.node_tree.nodes["Background"].inputs["Strength"].default_value = 0.65
    scene.world = world
    for name, location, energy in (
        ("Key", (-4, -5, 8), 3.0),
        ("Fill", (5, -2, 5), 1.5),
    ):
        data = bpy.data.lights.new(name, "SUN")
        data.energy = energy
        light = bpy.data.objects.new(name, data)
        light.location = location
        point_at(light, target)
        scene.collection.objects.link(light)
    return scene


def heading_rigs(armature, mesh, camera, headings, spacing):
    right = camera.matrix_world.to_quaternion() @ Vector((1, 0, 0))
    base_location = armature.location.copy()
    rigs = []
    for heading in range(headings):
        if heading == 0:
            rig = armature
            body = mesh
        else:
            rig = armature.copy()
            rig.data = armature.data.copy()
            body = mesh.copy()
            body.data = mesh.data
            body.parent = rig
            for modifier in body.modifiers:
                if modifier.type == "ARMATURE":
                    modifier.object = rig
            bpy.context.scene.collection.objects.link(rig)
            bpy.context.scene.collection.objects.link(body)
        offset = (heading - (headings - 1) / 2) * spacing
        rig.location = base_location + right * offset
        rig.rotation_euler[2] = heading * math.tau / headings
        rig.animation_data_create()
        rigs.append(rig)
    return rigs


def render_sheet(recipe, frame_size, headings, rows, root, output):
    armature, mesh = load_blend_model(inside(root, recipe["blend"]))
    scene = configure_scene(frame_size, headings, recipe["camera"])
    cell_scale = number(
        recipe["camera"].get("orthographicScale", 4.4),
        0.5,
        50,
        "camera.orthographicScale",
    )
    rigs = heading_rigs(
        armature,
        mesh,
        scene.camera,
        headings,
        cell_scale,
    )
    width = frame_size * headings
    height = frame_size * rows
    pixels = array("f", [0]) * (width * height * 4)
    row_index = 0

    with tempfile.TemporaryDirectory(prefix="sdr-directional-sprite-") as temporary:
        for animation in recipe["animations"]:
            if set(animation) != {"action", "frames", "name"} or not animation["frames"]:
                raise ValueError("animation recipe fields are invalid")
            action = find_action(animation["action"])
            for rig in rigs:
                rig.animation_data.action = action
            for source_frame in animation["frames"]:
                scene.frame_set(int(number(source_frame, action.frame_range[0], action.frame_range[1], "animation frame")))
                destination_y = height - (row_index + 1) * frame_size
                scene.render.filepath = str(Path(temporary) / f"row-{row_index:04d}.png")
                bpy.context.view_layer.update()
                bpy.ops.render.render(write_still=True)
                rendered = bpy.data.images.load(scene.render.filepath, check_existing=False)
                strip = array("f", rendered.pixels[:])
                bpy.data.images.remove(rendered)
                if len(strip) != width * frame_size * 4:
                    raise ValueError("Blender rendered an unexpected strip size")
                target_start = destination_y * width * 4
                pixels[target_start : target_start + len(strip)] = strip
                row_index += 1

    sheet = bpy.data.images.new("Directional sprite sheet", width=width, height=height, alpha=True)
    sheet.pixels.foreach_set(pixels)
    sheet.filepath_raw = str(output)
    sheet.file_format = "PNG"
    output.parent.mkdir(parents=True, exist_ok=True)
    sheet.save()


def main():
    args = arguments()
    recipe_path = Path(args.recipe).resolve()
    root = Path(args.source_root).resolve()
    output = Path(args.output).resolve()
    recipe, frame_size, headings, rows = load_recipe(recipe_path)
    render_sheet(recipe, frame_size, headings, rows, root, output)
    print("SDR_SPRITE_RESULT=" + json.dumps({
        "headings": headings,
        "output": str(output),
        "rows": rows,
    }))


main()
