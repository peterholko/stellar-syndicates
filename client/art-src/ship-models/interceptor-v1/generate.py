"""Build a first-pass 3D Interceptor from the approved 2D hull references.

Run from the repository root:
  /Applications/Blender.app/Contents/MacOS/Blender --background --python client/art-src/ship-models/interceptor-v1/generate.py
"""

import math
from pathlib import Path

import bpy
from mathutils import Vector


ROOT = Path(__file__).resolve().parent
for item in list(bpy.data.objects):
    bpy.data.objects.remove(item, do_unlink=True)


def material(name, color, metallic=0.0, roughness=0.5, emission=None):
    result = bpy.data.materials.new(name)
    result.diffuse_color = (*color, 1.0)
    result.use_nodes = True
    result.use_backface_culling = False
    shader = result.node_tree.nodes.get("Principled BSDF")
    shader.inputs["Base Color"].default_value = (*color, 1.0)
    shader.inputs["Metallic"].default_value = metallic
    shader.inputs["Roughness"].default_value = roughness
    if emission:
        shader.inputs["Emission Color"].default_value = (*color, 1.0)
        shader.inputs["Emission Strength"].default_value = emission
    return result


graphite = material("01 · gunmetal structural frame", (0.055, 0.075, 0.095), .78, .38)
dark_armor = material("02 · charcoal armor", (0.13, 0.17, 0.21), .74, .34)
silver = material("03 · brushed titanium", (0.56, 0.63, 0.68), .73, .29)
light_silver = material("04 · pale armor face", (0.77, 0.82, 0.85), .68, .30)
brass = material("05 · restrained brass hardware", (0.52, 0.36, 0.16), .83, .29)
glass = material("06 · dark blue-black canopy", (0.013, 0.027, 0.046), .48, .13)
glass_sheen = material("07 · canopy reflection", (0.12, 0.23, 0.30), .38, .20)
cyan = material("08 · cyan running light", (0.005, 0.69, 1.0), .12, .20, 7.0)
engine_blue = material("09 · engine core", (0.015, 0.79, 1.0), .10, .21, 10.0)
nozzle = material("10 · heat-treated nozzle", (0.085, 0.12, 0.16), .85, .26)


def finish(obj, mat, bevel=0.0):
    obj.data.materials.append(mat)
    if bevel:
        mod = obj.modifiers.new("machined edge radius", "BEVEL")
        mod.width = bevel
        mod.segments = 2
        mod.limit_method = "ANGLE"
        mod.angle_limit = math.radians(20)
        normals = obj.modifiers.new("weighted face normals", "WEIGHTED_NORMAL")
        normals.keep_sharp = True
    return obj


def polygon_prism(name, outline, z_low, z_high, mat, bevel=0.0):
    # Ensure a consistent upward top normal for both mirrored wings.
    area = sum(outline[i][0] * outline[(i + 1) % len(outline)][1]
               - outline[(i + 1) % len(outline)][0] * outline[i][1]
               for i in range(len(outline)))
    if area < 0:
        outline = list(reversed(outline))
    count = len(outline)
    verts = [(x, y, z_low) for x, y in outline] + [(x, y, z_high) for x, y in outline]
    faces = [tuple(reversed(range(count))), tuple(range(count, 2 * count))]
    faces += [(i, (i + 1) % count, (i + 1) % count + count, i + count) for i in range(count)]
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata(verts, [], faces)
    mesh.update()
    obj = bpy.data.objects.new(name, mesh)
    bpy.context.collection.objects.link(obj)
    return finish(obj, mat, bevel)


def box(name, location, size, mat, bevel=0.0):
    bpy.ops.mesh.primitive_cube_add(size=1, location=location)
    obj = bpy.context.object
    obj.name = name
    obj.dimensions = size
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    return finish(obj, mat, bevel)


def beam(name, start, end, width, depth, mat, bevel=0.0):
    a, b = Vector(start), Vector(end)
    midpoint = (a + b) / 2
    obj = box(name, midpoint, (width, (b - a).length, depth), mat, bevel)
    obj.rotation_euler = (b - a).to_track_quat("Y", "Z").to_euler()
    return obj


def cylinder_y(name, x, y, z, radius, length, mat, vertices=24, bevel=0.0):
    bpy.ops.mesh.primitive_cylinder_add(vertices=vertices, radius=radius, depth=length,
                                        location=(x, y, z), rotation=(math.pi / 2, 0, 0))
    obj = bpy.context.object
    obj.name = name
    return finish(obj, mat, bevel)


def cone_y(name, x, y, z, radius_a, radius_b, length, mat, vertices=24):
    bpy.ops.mesh.primitive_cone_add(vertices=vertices, radius1=radius_a,
                                    radius2=radius_b, depth=length,
                                    location=(x, y, z), rotation=(math.pi / 2, 0, 0))
    obj = bpy.context.object
    obj.name = name
    return finish(obj, mat, .025)


def torus_y(name, x, y, z, radius, thickness, mat):
    bpy.ops.mesh.primitive_torus_add(major_segments=32, minor_segments=8,
                                     location=(x, y, z), rotation=(math.pi / 2, 0, 0),
                                     major_radius=radius, minor_radius=thickness)
    obj = bpy.context.object
    obj.name = name
    return finish(obj, mat)


def mirror(points, sign):
    return [(sign * x, y) for x, y in points]


# Underbody and the long chisel-nosed central fuselage.
polygon_prism("central continuous hull", [
    (0, 6.50), (.13, 6.33), (.55, 4.00), (.82, 2.80),
    (1.08, 1.25), (1.18, -.75), (.97, -2.85), (.68, -4.92),
    (-.68, -4.92), (-.97, -2.85), (-1.18, -.75), (-1.08, 1.25),
    (-.82, 2.80), (-.55, 4.00), (-.13, 6.33),
], .08, .90, graphite, .11)
polygon_prism("nose · titanium blade", [
    (0, 6.47), (.11, 6.27), (.47, 4.03), (.65, 2.93),
    (.49, 2.52), (0, 2.76), (-.49, 2.52), (-.65, 2.93),
    (-.47, 4.03), (-.11, 6.27),
], .90, 1.14, light_silver, .045)
polygon_prism("central aft spine", [
    (-.43, -.72), (.43, -.72), (.64, -1.45), (.54, -4.70),
    (-.54, -4.70), (-.64, -1.45),
], .91, 1.14, silver, .055)
polygon_prism("spine center dark inlet", [
    (-.16, -1.33), (.16, -1.33), (.19, -3.65),
    (-.19, -3.65),
], 1.145, 1.16, dark_armor, .02)


# The split-delta wings have actual thickness, a recessed dark perimeter and
# separate raised armor plates. Open space remains around the engine roots.
wing = [
    (.87, 1.34), (1.57, .69), (3.11, -.89), (5.72, -2.80),
    (5.90, -3.12), (5.92, -3.72), (5.60, -3.81),
    (4.74, -3.34), (3.25, -2.40), (2.06, -1.96),
    (1.78, -2.20), (1.58, -3.07), (1.11, -3.24),
    (.89, -2.58),
]
primary_panel = [
    (1.31, .63), (2.90, -.92), (5.35, -2.78),
    (5.47, -3.18), (4.79, -2.96), (3.24, -2.04),
    (2.02, -1.47), (1.65, -1.54), (1.44, -1.07),
]
aft_panel = [
    (1.78, -1.71), (3.18, -2.22), (4.80, -3.15),
    (5.38, -3.49), (4.56, -3.26), (2.84, -2.60),
    (1.83, -2.36),
]
for side, label in [(1, "starboard"), (-1, "port")]:
    polygon_prism(f"{label} wing structural slab", mirror(wing, side), .19, .62, graphite, .095)
    polygon_prism(f"{label} swept top armor", mirror(primary_panel, side), .61, .72,
                  light_silver, .025)
    polygon_prism(f"{label} trailing armor", mirror(aft_panel, side), .62, .69,
                  silver, .02)
    polygon_prism(f"{label} outer gunmetal tip", mirror([
        (5.48, -2.85), (5.89, -3.08), (5.90, -3.74),
        (5.60, -3.78), (5.43, -3.43),
    ], side), .60, .81, dark_armor, .035)
    polygon_prism(f"{label} cyan wingtip strip", mirror([
        (5.70, -3.15), (5.77, -3.17), (5.79, -3.62), (5.72, -3.61),
    ], side), .813, .827, cyan, .007)
    beam(f"{label} leading-edge dark rail",
         (side * 1.72, .46, .75), (side * 5.32, -2.68, .75),
         .072, .042, dark_armor, .012)
    polygon_prism(f"{label} brass wing fitting", mirror([
        (2.94, -1.25), (3.23, -1.48), (3.14, -1.67), (2.87, -1.43),
    ], side), .727, .745, brass, .006)


# Raised shoulder cheeks, intake vents and compact forward weapon mounts.
for side, label in [(1, "starboard"), (-1, "port")]:
    polygon_prism(f"{label} cockpit cheek armor", mirror([
        (.69, 2.55), (1.07, 1.61), (1.12, -.55),
        (.79, -1.14), (.61, -.36), (.72, 1.28),
    ], side), .91, 1.17, silver, .055)
    polygon_prism(f"{label} recessed intake", mirror([
        (.88, .28), (1.13, .07), (1.12, -.54), (.90, -.42),
    ], side), 1.174, 1.187, graphite, .01)
    for i in range(4):
        y = .02 - i * .145
        beam(f"{label} intake vane {i + 1}",
             (side * .93, y, 1.204), (side * 1.09, y - .025, 1.204),
             .025, .025, dark_armor, .004)
    # Recessed gun paired with a short fairing beside the canopy.
    cylinder_y(f"{label} gun barrel", side * 1.21, 1.83, 1.07,
               .105, 1.12, nozzle, 12, .015)
    box(f"{label} gun root housing", (side * 1.21, 1.31, 1.05),
        (.28, .58, .25), dark_armor, .06)
    polygon_prism(f"{label} gun optical slit", mirror([
        (1.16, 1.60), (1.25, 1.60), (1.25, 2.01), (1.16, 2.01),
    ], side), 1.185, 1.199, cyan, .012)
    polygon_prism(f"{label} root brass vent", mirror([
        (1.00, -1.18), (1.24, -1.20), (1.40, -2.02), (1.22, -2.00),
    ], side), .89, .93, brass, .015)


# Tinted canopy in a raised brass rim, with actual side faces and metal frames.
canopy_rim = [
    (0, 2.63), (.50, 2.35), (.69, 1.48), (.67, -.01),
    (.45, -.86), (0, -1.02), (-.45, -.86), (-.67, -.01),
    (-.69, 1.48), (-.50, 2.35),
]
canopy_glass = [
    (0, 2.49), (.39, 2.25), (.55, 1.46), (.54, .02),
    (.34, -.71), (0, -.86), (-.34, -.71), (-.54, .02),
    (-.55, 1.46), (-.39, 2.25),
]
polygon_prism("canopy restrained brass sill", canopy_rim, 1.15, 1.22, brass, .028)
polygon_prism("canopy dark glazed volume", canopy_glass, 1.20, 1.64, glass, .10)
for y, half_width in [(1.68, .53), (.31, .52)]:
    beam("canopy segmented cross frame", (-half_width, y, 1.664),
         (half_width, y, 1.664), .055, .038, dark_armor, .01)
for side in [-1, 1]:
    beam("canopy longitudinal frame", (side * .35, 2.26, 1.65),
         (side * .50, -.68, 1.65), .040, .038, dark_armor, .01)
polygon_prism("canopy forward reflection", [
    (-.12, 2.17), (.13, 2.17), (.24, 1.93), (-.08, 1.93),
], 1.646, 1.654, glass_sheen, .005)


# Two large independent rear engine nacelles, with layered aft nozzle throats.
for side, label in [(1, "starboard"), (-1, "port")]:
    x = side * 1.36
    z = .58
    cone_y(f"{label} forward engine taper", x, -2.27, z,
           .50, .34, 1.40, dark_armor)
    cylinder_y(f"{label} titanium engine casing", x, -3.75, z,
               .59, 2.65, silver, 32, .045)
    cylinder_y(f"{label} aft engine collar", x, -5.03, z,
               .62, .63, graphite, 32, .035)
    torus_y(f"{label} brass nozzle retaining ring", x, -5.32, z, .52, .055, brass)
    cylinder_y(f"{label} black nozzle cavity", x, -5.40, z,
               .48, .20, nozzle, 32, .015)
    torus_y(f"{label} inner nozzle throat", x, -5.525, z, .37, .085, dark_armor)
    cylinder_y(f"{label} luminous ion core", x, -5.555, z,
               .31, .035, engine_blue, 32)
    torus_y(f"{label} inner cyan annulus", x, -5.583, z, .215, .028, cyan)
    polygon_prism(f"{label} engine top cyan bar", mirror([
        (1.31, -2.70), (1.41, -2.70), (1.41, -3.76), (1.31, -3.76),
    ], side), 1.167, 1.183, cyan, .012)
    box(f"{label} bar front brass fastener", (x, -2.59, 1.17),
        (.19, .15, .07), brass, .02)
    beam(f"{label} engine cowl seam", (x - side * .37, -2.81, 1.04),
         (x - side * .37, -4.84, 1.04), .026, .018, dark_armor, .003)


# Small signature cyan spine elements and restrained rear access panels.
polygon_prism("nose cyan sensor slit", [
    (-.050, 4.22), (.050, 4.22), (.050, 4.88), (-.050, 4.88),
], 1.145, 1.157, cyan, .008)
polygon_prism("aft dorsal cyan slit", [
    (-.055, -.93), (.055, -.93), (.055, -1.48), (-.055, -1.48),
], 1.172, 1.185, cyan, .007)
box("rear access latch", (0, -4.11, 1.185), (.26, .18, .045), brass, .012)


# Model is authored along +Y (nose), with Z up. These empty nodes make the
# primary moving parts easy to locate in downstream 3D tools.
for name, location in [
    ("PIVOT · center of mass", (0, -.35, .55)),
    ("MOUNT · port weapon", (-1.21, 2.35, 1.07)),
    ("MOUNT · starboard weapon", (1.21, 2.35, 1.07)),
    ("MOUNT · port engine", (-1.36, -5.59, .58)),
    ("MOUNT · starboard engine", (1.36, -5.59, .58)),
]:
    empty = bpy.data.objects.new(name, None)
    empty.location = location
    bpy.context.collection.objects.link(empty)


def point_camera(location, target=(0, 0, .5), lens=43):
    camera = bpy.data.objects.get("Preview camera")
    if not camera:
        bpy.ops.object.camera_add()
        camera = bpy.context.object
        camera.name = "Preview camera"
    camera.location = location
    camera.rotation_euler = (Vector(target) - camera.location).to_track_quat("-Z", "Y").to_euler()
    camera.data.type = "ORTHO"
    camera.data.ortho_scale = lens
    bpy.context.scene.camera = camera
    return camera


def area_light(name, location, energy, size, color=(1, 1, 1)):
    bpy.ops.object.light_add(type="AREA", location=location)
    lamp = bpy.context.object
    lamp.name = name
    lamp.data.energy = energy
    lamp.data.shape = "DISK"
    lamp.data.size = size
    lamp.data.color = color
    lamp.rotation_euler = (Vector((0, 0, .3)) - lamp.location).to_track_quat("-Z", "Y").to_euler()


area_light("large cool key", (-7, 7, 14), 2600, 9)
area_light("soft aft fill", (7, -8, 9), 1900, 8, (.62, .82, 1))
area_light("bright top reflection", (0, 3, 15), 1500, 7)
scene = bpy.context.scene
scene.render.engine = "CYCLES"
scene.cycles.samples = 40
scene.render.resolution_x = 1100
scene.render.resolution_y = 1100
scene.render.resolution_percentage = 100
scene.render.image_settings.file_format = "PNG"
scene.render.film_transparent = False
scene.world.color = (.005, .015, .03)
scene.view_settings.view_transform = "AgX"
scene.render.filepath = str(ROOT / "interceptor-preview.png")

point_camera((15, -16, 15), lens=17)
bpy.ops.wm.save_as_mainfile(filepath=str(ROOT / "interceptor.blend"))

# Save only model objects in the interchange file: no preview lights or camera.
for obj in bpy.context.scene.objects:
    obj.select_set(obj.type == "MESH" or obj.type == "EMPTY")
bpy.context.view_layer.objects.active = next(obj for obj in scene.objects if obj.type == "MESH")
bpy.ops.export_scene.gltf(filepath=str(ROOT / "interceptor.glb"), export_format="GLB",
                          use_selection=True, export_apply=True,
                          export_lights=False, export_cameras=False)

for filename, location, scale in [
    ("interceptor-preview.png", (15, -16, 15), 17),
    ("interceptor-top.png", (0, 0, 19), 15),
    ("interceptor-side.png", (18, -1, 4.0), 17),
]:
    point_camera(location, lens=scale)
    scene.render.filepath = str(ROOT / filename)
    bpy.ops.render.render(write_still=True)

print(f"Wrote {ROOT / 'interceptor.glb'} and three previews")
