"""Authority Nexus: deterministic reference-textured geometry; no Blender.

Authoring axes are X/Y in the station plane and Z up. Export is glTF Y-up.
Rebuild: python3 build_model.py. Units are relative: the outer hull is 20 wide.
"""
from pathlib import Path
from collections import defaultdict
import json
import math
import shutil
import struct
import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent
ART = ROOT.parent.parent
REFERENCE = ART / 'authority-nexus-reference-v1/authority-nexus-reference.png'
CONCEPT = ART / 'authority-nexus-anchor-v1/authority-nexus-anchor.png'
if not REFERENCE.exists():
    REFERENCE = ROOT/'reference.png'
if not CONCEPT.exists():
    CONCEPT = ROOT/'concept.png'
TAU = math.tau
TEXTURES = ['authority-nexus-basecolor.png', 'authority-nexus-metallic-roughness.png',
            'authority-nexus-emissive.png', 'authority-nexus-ivory-panels.png',
            'authority-nexus-ivory-metallic-roughness.png', 'authority-nexus-brass-ring.png',
            'authority-nexus-brass-ring-metallic-roughness.png', 'authority-nexus-power-cells.png',
            'authority-nexus-power-cells-metallic-roughness.png', 'authority-nexus-power-cells-emissive.png']
parts = []
materials = []


def unit(v):
    v = np.asarray(v, dtype=float)
    return v / max(np.linalg.norm(v), 1e-12)


def material(name, color=(1, 1, 1), metal=.4, rough=.5, emission=None, texture=False):
    pbr = dict(baseColorFactor=[*color, 1], metallicFactor=metal, roughnessFactor=rough)
    item = dict(name=name, pbrMetallicRoughness=pbr)
    if texture:
        pbr.update(baseColorTexture={'index': 0}, metallicRoughnessTexture={'index': 1})
        item.update(emissiveTexture={'index': 2}, emissiveFactor=[.35, .35, .35])
    if emission is not None:
        item['emissiveFactor'] = list(emission)
    materials.append(item)
    return len(materials)-1


ATLAS = material('Reference armor and port architecture', metal=1, rough=1, texture=True)
BELLY = material('Ventral reference armor and radiators', metal=1, rough=1, texture=True)
DARK = material('Graphite structure', (.10, .12, .15), .55, .48)
materials[DARK]['pbrMetallicRoughness']['baseColorTexture'] = {'index': 3}
BRASS = material('Brushed brass fittings', (.90, .88, .82), 1, 1)
materials[BRASS]['pbrMetallicRoughness'].update(
    baseColorTexture={'index': 5}, metallicRoughnessTexture={'index': 6})
IVORY = material('Ivory titanium panel armor', metal=1, rough=1)
materials[IVORY]['pbrMetallicRoughness'].update(
    baseColorTexture={'index': 3}, metallicRoughnessTexture={'index': 4})
CYAN = material('Powered cyan stabilizer field', (.025, .18, .32), .05, .25, (.07, .68, 1.0))
AMBER = material('Warm port glazing', (.20, .12, .042), .35, .22, (.32, .17, .052))
PORTAL = material('Violet wormhole energy — independent surface', metal=0, rough=1)
materials[PORTAL]['pbrMetallicRoughness']['baseColorTexture'] = {'index': 0}
materials[PORTAL]['extensions'] = {'KHR_materials_unlit': {}}
materials[PORTAL]['doubleSided'] = True
COLLAR = material('Machined brass containment collars', metal=1, rough=1)
materials[COLLAR]['pbrMetallicRoughness'].update(
    baseColorTexture={'index': 5}, metallicRoughnessTexture={'index': 6})
POWER = material('Powered cyan reactor cells', metal=1, rough=1, emission=(.85, .85, .85))
materials[POWER]['pbrMetallicRoughness'].update(
    baseColorTexture={'index': 7}, metallicRoughnessTexture={'index': 8})
materials[POWER]['emissiveTexture'] = {'index': 9}


def texture_files():
    folder = ROOT/'textures'
    folder.mkdir(exist_ok=True)
    shutil.copy2(REFERENCE, folder/'authority-nexus-basecolor.png')
    image = np.asarray(Image.open(REFERENCE).convert('RGB'), dtype=float)/255
    lo, hi = image.min(axis=2), image.max(axis=2)
    r, g, b = image.transpose(2, 0, 1)
    cyan = np.clip((b-r-.08)*3, 0, 1) * np.clip(g*1.6, 0, 1)
    amber = np.clip((r-b-.1)*2, 0, 1) * np.clip((hi-.52)*3, 0, 1)
    em = image*(cyan*.7+amber*.22)[:, :, None]
    mr = np.ones_like(image)
    mr[:, :, 1] = np.clip(.54+(1-hi)*.22, 0, 1)
    mr[:, :, 2] = np.clip(.35+(hi-lo)*.32, 0, 1)
    Image.fromarray((np.clip(em, 0, 1)*255).astype('uint8')).save(folder/'authority-nexus-emissive.png')
    Image.fromarray((mr*255).astype('uint8')).save(folder/'authority-nexus-metallic-roughness.png')
    # The pylon sides have their own flat armor texture; projecting them from
    # the reference elevations would include background and foreshorten panels.
    armor = np.asarray(Image.open(folder/TEXTURES[3]).convert('RGB'), dtype=float)/255
    brightness = armor.mean(axis=2)
    brass = np.clip((armor[:, :, 0]-armor[:, :, 2]-.065)*5, 0, 1)
    armor_mr = np.ones_like(armor)
    armor_mr[:, :, 1] = np.clip(.48+(1-brightness)*.20-brass*.10, 0, 1)
    armor_mr[:, :, 2] = np.clip(.30+brass*.45, 0, 1)
    Image.fromarray((armor_mr*255).astype('uint8')).save(folder/TEXTURES[4])
    ring = np.asarray(Image.open(folder/TEXTURES[5]).convert('RGB'), dtype=float)/255
    luminance = ring@np.array([.2126, .7152, .0722])
    recessed = np.clip((.30-luminance)*4, 0, 1)
    ring_mr = np.ones_like(ring)
    ring_mr[:, :, 1] = np.clip(.37+recessed*.18+(1-luminance)*.06, 0, 1)
    ring_mr[:, :, 2] = .78-recessed*.24
    Image.fromarray((ring_mr*255).astype('uint8')).save(folder/TEXTURES[6])
    # Select the blue cells for emission; their dark separators remain visible.
    cells = np.asarray(Image.open(folder/TEXTURES[7]).convert('RGB'), dtype=float)/255
    r, g, b = cells.transpose(2, 0, 1)
    energy = np.clip((b-r-.12)*2.6, 0, 1)*np.clip((g-.12)*1.9, 0, 1)
    cells_mr = np.ones_like(cells)
    cells_mr[:, :, 1] = .32+.20*(1-energy)
    cells_mr[:, :, 2] = .12+.38*(1-energy)
    Image.fromarray((cells_mr*255).astype('uint8')).save(folder/TEXTURES[8])
    Image.fromarray((cells*(energy*.82)[:, :, None]*255).astype('uint8')).save(folder/TEXTURES[9])
    if REFERENCE.resolve() != (ROOT/'reference.png').resolve():
        shutil.copy2(REFERENCE, ROOT/'reference.png')
    if CONCEPT.resolve() != (ROOT/'concept.png').resolve():
        shutil.copy2(CONCEPT, ROOT/'concept.png')


def top_uv(p):
    radius = math.hypot(p[0], p[1])
    # Register the opening exactly, then fit the actual deck rather than the
    # pylon-tip bounding circle. This excludes the sheet's white background.
    pixels = radius*30.5 if radius <= 4.02 else 122.61+(min(radius, 9.9)-4.02)*23.8
    scale = pixels/max(radius, 1e-9)
    return ((395+p[0]*scale)/1536, (320-p[1]*scale)/1024)


def bottom_uv(p):
    radius = math.hypot(p[0], p[1])
    pixels = radius*29.6 if radius <= 4.20 else 124.32+(min(radius, 9.9)-4.20)*22.8
    scale = pixels/max(radius, 1e-9)
    return ((1133-p[0]*scale)/1536, (320-p[1]*scale)/1024)


def hull_uv(p, n):
    if n[2] > .5:
        return top_uv(p)
    if n[2] < -.5:
        return bottom_uv(p)
    # Cylindrical elevation mapping stays in the reference's occupied hull band.
    a = math.atan2(p[1], p[0])
    u = 44 + ((a/TAU+.5) % 1)*438
    v = np.clip(850-p[2]*29, 798, 899)
    return (u/1536, v/1024)


def triangulate(vertices):
    if len(vertices) == 3:
        return [(0, 1, 2)]
    normal = np.sum(np.cross(vertices, np.roll(vertices, -1, axis=0)), axis=0)
    xy = np.delete(vertices, np.argmax(np.abs(normal)), axis=1)
    area = np.sum(xy[:, 0]*np.roll(xy[:, 1], -1)-np.roll(xy[:, 0], -1)*xy[:, 1])
    sign = 1 if area >= 0 else -1
    cross = lambda a, b: a[0]*b[1]-a[1]*b[0]
    remaining = list(range(len(vertices)))
    result = []
    while len(remaining) > 3:
        for k, b in enumerate(remaining):
            a, c = remaining[k-1], remaining[(k+1) % len(remaining)]
            if sign*cross(xy[b]-xy[a], xy[c]-xy[b]) < 1e-12:
                continue
            if any(all(sign*q > 1e-10 for q in (
                cross(xy[b]-xy[a], xy[j]-xy[a]),
                cross(xy[c]-xy[b], xy[j]-xy[b]),
                cross(xy[a]-xy[c], xy[j]-xy[c]))) for j in remaining if j not in (a, b, c)):
                continue
            result.append((a, b, c))
            remaining.pop(k)
            break
        else:
            raise ValueError('Non-simple or degenerate polygon')
    result.append(tuple(remaining))
    return result


def panel_face_uv(points, normal, mat):
    """Unwrap a whole planar face with constant texel density and no pinching."""
    edges = np.roll(points, -1, axis=0)-points
    horizontal = unit(edges[np.argmax(np.linalg.norm(edges, axis=1))])
    vertical = unit(np.cross(normal, horizontal))
    xy = points@np.column_stack([horizontal, vertical])
    center = (xy.min(axis=0)+xy.max(axis=0))/2
    texture = materials[mat]['pbrMetallicRoughness']['baseColorTexture']['index']
    # One brass panel in the lower atlas strip, or the square ivory armor map.
    uv_center, limits, aspect = (np.array([.10, .75]), np.array([.18, .48]), np.array([3, 1])) if texture == 5 else (
        np.array([.5, .5]), np.array([.94, .94]), np.array([1, 1]))
    scale = np.min(limits*aspect/np.maximum(np.ptp(xy, axis=0), 1e-12))
    return uv_center+(xy-center)*scale/aspect


def mesh(name, vertices, faces, mat=ATLAS, assembly='structure', mapping=None,
         closed=True, smooth=False):
    v = np.asarray(vertices, dtype=float)
    faces = [list(f) for f in faces]
    volume = sum(np.dot(v[f[0]], np.cross(v[f[j]], v[f[j+1]]))
                 for f in faces for j in range(1, len(f)-1))/6
    if closed and volume < 0:
        faces = [f[::-1] for f in faces]
    normals = [unit(np.sum(np.cross(v[f], np.roll(v[f], -1, axis=0)), axis=0)) for f in faces]
    adjacent = [[] for _ in v]
    for fi, f in enumerate(faces):
        for vi in f:
            adjacent[vi].append(fi)
    groups = {}
    for fi, face in enumerate(faces):
        n = normals[fi]
        chosen = BELLY if mat == ATLAS and n[2] < -.5 and assembly == 'structure' else mat
        triangles = triangulate(v[face])
        texture = materials[chosen]['pbrMetallicRoughness'].get('baseColorTexture', {}).get('index')
        if mapping is None and texture in (3, 5):
            face_uv = panel_face_uv(v[face], n, chosen)
        else:
            face_uv = np.array([np.clip(mapping(v[vi], n) if mapping else hull_uv(v[vi], n), .001, .999)
                                for vi in face])
        # Projection of an elevation or underside can collapse a perpendicular
        # cap to a line. Give those faces real local armor UVs instead.
        if chosen in (ATLAS, BELLY):
            uv_triangles = face_uv[np.asarray(triangles)]
            a, b = uv_triangles[:, 1]-uv_triangles[:, 0], uv_triangles[:, 2]-uv_triangles[:, 0]
            if np.abs(a[:, 0]*b[:, 1]-a[:, 1]*b[:, 0]).min() < 1e-10:
                chosen = DARK if name.startswith(('port.glass.', 'belly.radiator.')) else IVORY
                face_uv = panel_face_uv(v[face], n, chosen)
        g = groups.setdefault(chosen, dict(position=[], normal=[], uv=[], indices=[]))
        start = len(g['position'])
        for local_index, vi in enumerate(face):
            vn = n
            if smooth:
                vn = unit(sum((normals[i] for i in adjacent[vi] if normals[i]@n > .75), np.zeros(3)))
            g['position'].append(v[vi])
            g['normal'].append(vn)
            g['uv'].append(face_uv[local_index])
        for triangle in triangles:
            g['indices'].extend(start+i for i in triangle)
    parts.append(dict(name=name, assembly=assembly, groups=groups, closed=closed))


def polar(radius, angle, z):
    return np.array([radius*math.cos(angle), radius*math.sin(angle), z])


def annulus(name, profile, segments=32, start=0, sweep=TAU, mat=ATLAS,
            assembly='structure', mapping=None):
    """Closed polygon of revolution, or capped annular sector."""
    full = abs(sweep-TAU) < 1e-7
    count = segments if full else segments+1
    vertices = [polar(r, start+sweep*i/segments, z) for r, z in profile for i in range(count)]
    faces = []
    rings = len(profile)
    for row in range(rings):
        other = (row+1) % rings
        for i in range(segments):
            nxt = (i+1) % count
            faces.append((row*count+i, row*count+nxt, other*count+nxt, other*count+i))
    if not full:
        faces.extend([tuple(k*count for k in range(rings)),
                      tuple(k*count+segments for k in reversed(range(rings)))])
    mesh(name, vertices, faces, mat, assembly, mapping)


def cylinder(name, a, b, radius, mat, assembly='structure', segments=10, radius2=None, uv_style=None):
    a, b = np.asarray(a), np.asarray(b)
    d = unit(b-a)
    helper = [0, 0, 1] if abs(d[2]) < .9 else [1, 0, 0]
    right = unit(np.cross(d, helper))
    up = unit(np.cross(d, right))
    radius2 = radius if radius2 is None else radius2
    v = [c+r*(math.cos(i*TAU/segments)*right+math.sin(i*TAU/segments)*up)
         for c, r in [(a, radius), (b, radius2)] for i in range(segments)]
    f = [tuple(reversed(range(segments))), tuple(range(segments, 2*segments))]
    f.extend((i, (i+1) % segments, (i+1) % segments+segments, i+segments) for i in range(segments))
    mesh(name, v, f, mat, assembly, smooth=True)
    if uv_style is None:
        texture = materials[mat]['pbrMetallicRoughness'].get('baseColorTexture', {}).get('index')
        if texture in (3, 5):
            uv_style = 'brass' if texture == 5 else 'panel'
    if uv_style is not None:
        # Remap the existing face-local vertices, keeping positions and normals.
        # Planar caps avoid pinched center UVs; every wall quad crosses the
        # angular seam continuously, using a horizontally repeating sampler.
        g = parts[-1]['groups'][mat]
        length = np.linalg.norm(b-a)
        for start, center, cap_radius in [(0, a, radius), (segments, b, radius2)]:
            local = np.asarray(g['position'][start:start+segments])-center
            x, y = local@right/cap_radius, local@up/cap_radius
            vv = .25+.24*y if uv_style == 'brass' else .5+.48*y
            g['uv'][start:start+segments] = np.column_stack([.5+.48*x, vv]).tolist()
        repeats = max(1, round(TAU*radius/length/6)) if uv_style == 'brass' else 1
        for start in range(segments*2, len(g['position']), 4):
            local = np.asarray(g['position'][start:start+4])-a
            angles = np.unwrap(np.mod(np.arctan2(local@up, local@right), TAU))
            if angles.min() < -1e-8:
                angles += TAU
            height = np.clip(local@d/length, 0, 1)
            vv = .505+.49*height if uv_style == 'brass' else .01+.98*(1-height)
            g['uv'][start:start+4] = np.column_stack([angles/TAU*repeats, vv]).tolist()


def wrap_annular_band(part, mat, r0, r1, low, high, repeats=8):
    """Continuous angular UVs on both annular faces and both cylindrical walls."""
    g = part['groups'][mat]
    for start in range(0, len(g['position']), 4):
        points = np.asarray(g['position'][start:start+4])
        angles = np.unwrap(np.mod(np.arctan2(points[:, 1], points[:, 0]), TAU))
        if angles.min() < -1e-8:
            angles += TAU
        if abs(g['normal'][start][2]) > .5:
            v = .005+.49*(np.linalg.norm(points[:, :2], axis=1)-r0)/(r1-r0)
        else:
            v = .505+.49*(points[:, 2]-low)/(high-low)
        g['uv'][start:start+4] = np.column_stack([angles/TAU*repeats, v]).tolist()


def beam(name, a, b, width, depth, mat=DARK, assembly='structure'):
    a, b = np.asarray(a), np.asarray(b)
    d = unit(b-a)
    helper = [0, 0, 1] if abs(d[2]) < .9 else [1, 0, 0]
    right = unit(np.cross(d, helper))*width/2
    up = unit(np.cross(d, right))*depth/2
    v = [p+s*right+t*up for p in [a, b] for s, t in [(-1, -1), (1, -1), (1, 1), (-1, 1)]]
    mesh(name, v, [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4),
                  (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)], mat, assembly)


def radial_box(name, r0, r1, width, low, high, angle, mat=ATLAS, assembly='structure'):
    axis = polar(1, angle, 0)
    tangent = polar(1, angle+math.pi/2, 0)
    v = [axis*r+tangent*t+[0, 0, z] for z in [low, high]
         for r, t in [(r0, -width/2), (r1, -width/2), (r1, width/2), (r0, width/2)]]
    mesh(name, v, [(3, 2, 1, 0), (4, 5, 6, 7), (0, 1, 5, 4),
                  (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)], mat, assembly)


def pylon_rib(name, angle, side, assembly):
    # Swept load-bearing ribs flank the exposed field-generator core.
    rows = [(.44, 5.0, 9.87, .88), (1.05, 5.1, 9.55, .80),
            (1.40, 5.72, 9.12, .66), (2.10, 6.75, 8.77, .50),
            (3.15, 7.34, 8.57, .36), (4.36, 7.63, 8.41, .25),
            (4.65, 7.76, 8.30, .19)]
    axis, tangent = polar(1, angle, 0), polar(1, angle+math.pi/2, 0)
    v = []
    for z, r0, r1, width in rows:
        # Parallel extruded ribs keep their broad surfaces planar. A changing
        # tangential offset twists these very thin shells and creates pinholes.
        width = .36
        for r, t in [(r0, side*width), (r1, side*width),
                     (r1, side*(width+.14)), (r0, side*(width+.14))]:
            v.append(axis*r+tangent*t+[0, 0, z])
    f = [(3, 2, 1, 0)]
    for row in range(len(rows)-1):
        f.extend((row*4+i, row*4+(i+1) % 4, (row+1)*4+(i+1) % 4, (row+1)*4+i) for i in range(4))
    f.append(tuple(range(len(v)-4, len(v))))

    def armor_uv(p, normal):
        # Local planar coordinates give identical panel density on all four
        # towers, on both faces of each shell and across every height segment.
        r, t, z = float(p@axis), float(p@tangent)-side*.43, p[2]
        dominant = np.argmax([abs(normal@axis), abs(normal@tangent), abs(normal[2])])
        if dominant == 1:
            return (.02+.96*(r-5)/4.87, .98-.96*(z-.44)/4.21)
        if dominant == 2:
            return (.02+.96*(r-5)/4.87, .5-.96*t/4.21)
        return (.5+.96*t/4.87, .98-.96*(z-.44)/4.21)

    mesh(name, v, f, IVORY, assembly, mapping=armor_uv)
    # Fine brass trim follows the curved outer shoulder and catches edge light.
    for i, (a, b) in enumerate(zip(rows, rows[1:])):
        start = axis*a[1]+tangent*(side*.515)+[0, 0, a[0]+.015]
        end = axis*b[1]+tangent*(side*.515)+[0, 0, b[0]+.015]
        beam(name+f'.trim.{i}', start, end, .035, .035, BRASS, assembly)


def build_geometry():
    # Closed annular profiles leave a genuine through-hole when the energy is off.
    annulus('hull.load-bearing-annulus', [(4.20, -.75), (8.55, -1.25),
        (9.22, -.90), (9.65, -.37), (9.65, .02), (9.22, .22), (4.20, .22)])
    annulus('hull.outer-terrace', [(7.18, .16), (9.35, .16), (9.65, .26),
        (9.45, .44), (7.18, .44)])
    annulus('hull.inner-hall', [(4.70, -.1), (7.18, -.1), (7.28, .66),
        (7.10, .95), (6.14, 1.12), (5.02, .91), (4.70, .68)])
    annulus('containment.machine-collar', [(4.04, -.64), (4.61, -.64),
        (4.89, .66), (4.76, .94), (4.29, 1.02), (4.04, .76)])
    annulus('containment.inner-lip', [(4.01, .44), (4.12, .44),
        (4.12, .65), (4.01, .65)], mat=BRASS)
    wrap_annular_band(parts[-1], BRASS, 4.01, 4.12, .44, .65)
    annulus('containment.field-rim', [(3.985, .49), (4.02, .49),
        (4.02, .58), (3.985, .58)], mat=CYAN, assembly='containment-field', segments=64)
    annulus('hull.ventral-collar', [(4.20, -1.02), (4.66, -1.02),
        (4.66, -.65), (4.20, -.65)], mat=COLLAR)
    # Give each face continuous angular coordinates, including the 0/2pi seam.
    # The atlas repeats eight times around the circumference. Its upper strip
    # covers both annular faces; its lower strip wraps the inner/outer walls.
    # This only changes UVs; the original collar geometry remains untouched.
    wrap_annular_band(parts[-1], COLLAR, 4.20, 4.66, -1.02, -.65)

    for i in range(24):
        a = i*TAU/24
        # Raised individual cargo terraces interrupt the otherwise simple ring.
        annulus(f'dock.terrace.{i}', [(7.3, .41), (9.31, .41), (9.42, .54),
            (9.13, .63), (7.3, .54)], 2, a+.021, TAU/24-.042)
        annulus(f'port.glass.{i}', [(7.204, .35), (7.248, .35),
            (7.248, .69), (7.204, .69)], 2, a+.026, TAU/24-.052,
            ATLAS, mapping=lambda p, n: ((75+((math.atan2(p[1],p[0])/TAU+.5) % 1)*379)/1536,
                                       (829-(p[2]-.35)*32)/1024))
        # Outer deck seams and braces are geometry; fine grooves stay in textures.
        beam(f'dock.seam.{i}', polar(7.30, a, .57), polar(9.39, a, .59), .055, .055, IVORY)
        annulus(f'belly.radiator.{i}', [(7.54, -1.18), (8.96, -.85),
            (8.96, -.49), (7.54, -.65)], 2, a+.034, TAU/24-.068,
            BELLY, mapping=lambda p, n: bottom_uv(p))
        if i % 6 not in (0, 5):
            radial_box(f'cargo.container.{i}', 8.14, 8.66, .24, .65, .82, a+.13, ATLAS)
        if i % 2 == 0:
            # Explicit underside ribs give depth to the bottom reference texture.
            beam(f'belly.rib.{i}', polar(4.62, a, -.82), polar(8.77, a, -1.04), .18, .18, DARK)
            cylinder(f'coolant.{i}', polar(4.73, a+.026, -.9), polar(7.52, a+.026, -1.14),
                     .065, BRASS, segments=6)
        if i % 6 not in (0, 5):
            a0, b0 = polar(4.75, a+.04, .91), polar(4.75, a+.20, .91)
            cylinder(f'emitter.induction.{i}', a0, b0, .17, BRASS, segments=8)

    for index in range(4):
        a = index*math.pi/2
        group = f'stabilizer-{index+1}'
        axis, tangent = polar(1, a, 0), polar(1, a+math.pi/2, 0)
        radial_box(group+'.foundation', 5.0, 9.90, 1.98, .35, .68, a)
        pylon_rib(group+'.left-shell', a, -1, group)
        pylon_rib(group+'.right-shell', a, 1, group)
        cylinder(group+'.powered-core', polar(8.03, a, 1.09), polar(8.03, a, 4.50), .205,
                 POWER, group, 12, uv_style='power')
        for j in range(6):
            z = 1.32+j*.51
            cylinder(group+f'.coil.{j}', polar(8.03, a, z), polar(8.03, a, z+.115),
                     .365, COLLAR, group, 8, uv_style='brass')
        cylinder(group+'.crown', polar(8.03, a, 4.39), polar(8.03, a, 4.71), .21, IVORY, group, 10, .13)
        cylinder(group+'.beacon', polar(8.03, a, 4.70), polar(8.03, a, 4.90), .034, POWER, group, 6, uv_style='power')
        for side in [-1, 1]:
            path = [axis*r+tangent*side*.52+[0, 0, z]
                    for r, z in [(4.33, 1.02), (5.3, 1.10), (6.35, 1.24), (7.18, 1.66), (7.92, 2.37)]]
            for j, (p, q) in enumerate(zip(path, path[1:])):
                cylinder(group+f'.conduit-housing.{side}.{j}', p, q, .12, DARK, group, 8)
                # Thin luminous strip rides above the large mechanical conduit.
                cylinder(group+f'.power-feed.{side}.{j}', p+[0, 0, .099], q+[0, 0, .099],
                         .036, POWER, group, 6, uv_style='power')
                d = unit(q-p)
                cylinder(group+f'.coupling.{side}.{j}', p, p+d*.09, .137, COLLAR, group, 8, uv_style='brass')
        radial_box(group+'.emitter-mount', 4.02, 4.68, .66, .68, 1.19, a, DARK, group)
        for side in [-1, 1]:
            beam(group+f'.emitter-jaw.{side}', axis*4.56+tangent*side*.32+[0, 0, 1.18],
                 axis*3.95+tangent*side*.24+[0, 0, .83], .12, .15, IVORY, group)
            path = [axis*r+tangent*side*t+[0, 0, z] for r, t, z in [
                (4.02, .23, .92), (3.88, .15, .81), (3.78, .07, .64), (3.70, .0, .51)]]
            for j, (p, q) in enumerate(zip(path, path[1:])):
                cylinder(group+f'.field-tether.{side}.{j}', p, q, .027, CYAN, 'containment-field', 6)
        radial_box(group+'.ventral-power-housing', 4.78, 8.61, 1.2, -1.44, -.57, a,
                   BELLY)

    # A separate two-sided energy membrane can scale away, exposing a real hole.
    segments = 64
    profile = [(.25, -.44), (.80, -.35), (1.55, -.20), (2.30, .08),
               (3.18, .37), (4.00, .52)]
    vertices = [np.array([0, 0, -.47])]
    vertices.extend(polar(r, i*TAU/segments, z) for r, z in profile for i in range(segments))
    faces = [(0, 1+i, 1+(i+1) % segments) for i in range(segments)]
    for row in range(len(profile)-1):
        for i in range(segments):
            nxt = (i+1) % segments
            faces.append((1+row*segments+i, 1+(row+1)*segments+i,
                          1+(row+1)*segments+nxt, 1+row*segments+nxt))
    # Correct the membrane winding explicitly (it is intentionally not a solid).
    faces = [f if np.cross(np.asarray(vertices)[f[1]]-vertices[f[0]],
                          np.asarray(vertices)[f[2]]-vertices[f[0]])[2] > 0 else tuple(reversed(f)) for f in faces]
    mesh('wormhole.energy-membrane', vertices, faces, PORTAL, 'wormhole',
         lambda p, n: top_uv(p), closed=False, smooth=True)


def export(file_stem='authority-nexus', info_overrides=None):
    grouped = {}
    for part in parts:
        for mat, g in part['groups'].items():
            key = (part['assembly'], mat)
            merged = grouped.setdefault(key, dict(position=[], normal=[], uv=[], indices=[], components=[]))
            offset = len(merged['position'])
            merged['components'].append(dict(name=part['name'], indexStart=len(merged['indices']),
                                              indexCount=len(g['indices']), closed=part['closed']))
            for attr in ['position', 'normal', 'uv']:
                merged[attr].extend(g[attr])
            merged['indices'].extend(i+offset for i in g['indices'])
    binary = bytearray()
    doc = dict(asset={'version': '2.0', 'generator': 'Stellar Syndicates NumPy mesh pipeline; no Blender'},
               scene=0, scenes=[{'nodes': [0]}], nodes=[{'name': 'Authority Nexus', 'children': [],
               'extras': {'reference': '../../authority-nexus-reference-v1/authority-nexus-reference.png',
                          'role': 'Destination-side powered wormhole anchor'}}],
               meshes=[], materials=materials, buffers=[{}], bufferViews=[], accessors=[], images=[],
               textures=[], samplers=[{'magFilter': 9729, 'minFilter': 9987, 'wrapS': 33071, 'wrapT': 33071},
                                      {'magFilter': 9729, 'minFilter': 9987, 'wrapS': 10497, 'wrapT': 33071}],
               extensionsUsed=['KHR_materials_unlit'])

    def view(raw, target=None):
        while len(binary) % 4:
            binary.append(0)
        entry = dict(buffer=0, byteOffset=len(binary), byteLength=len(raw))
        binary.extend(raw)
        if target:
            entry['target'] = target
        doc['bufferViews'].append(entry)
        return len(doc['bufferViews'])-1

    def accessor(data, component, kind, target=None, bounds=False):
        a = np.asarray(data, dtype={5126: '<f4', 5123: '<u2', 5125: '<u4'}[component])
        acc = dict(bufferView=view(a.tobytes(), target), componentType=component, count=len(a), type=kind)
        if bounds:
            acc.update(min=a.min(axis=0).reshape(-1).tolist(), max=a.max(axis=0).reshape(-1).tolist())
        doc['accessors'].append(acc)
        return len(doc['accessors'])-1

    for filename in TEXTURES:
        doc['images'].append(dict(name=filename, mimeType='image/png',
                                  bufferView=view((ROOT/'textures'/filename).read_bytes())))
        doc['textures'].append({'source': len(doc['images'])-1, 'sampler': 1 if filename in TEXTURES[5:] else 0})
    assembly_nodes = {}
    points = []
    triangles = 0
    for (assembly, mat), g in grouped.items():
        if assembly not in assembly_nodes:
            assembly_nodes[assembly] = len(doc['nodes'])
            doc['nodes'].append(dict(name=assembly, children=[], extras={'assembly': assembly}))
            doc['nodes'][0]['children'].append(len(doc['nodes'])-1)
        packed = np.column_stack([g['position'], g['normal'], g['uv']]).astype('<f4')
        unique, inverse = np.unique(packed, axis=0, return_inverse=True)
        pos = unique[:, [0, 2, 1]]*np.array([1, 1, -1])
        normals = unique[:, [3, 5, 4]]*np.array([1, 1, -1])
        indices = inverse[g['indices']]
        primitive = dict(attributes={
            'POSITION': accessor(pos, 5126, 'VEC3', 34962, True),
            'NORMAL': accessor(normals, 5126, 'VEC3', 34962),
            'TEXCOORD_0': accessor(unique[:, 6:8], 5126, 'VEC2', 34962)},
            indices=accessor(indices, 5123 if len(pos) < 65536 else 5125, 'SCALAR', 34963), material=mat)
        name = assembly+' / '+materials[mat]['name']
        doc['meshes'].append(dict(name=name, primitives=[primitive],
                                  extras={'assembly': assembly, 'sourceComponents': g['components']}))
        doc['nodes'].append(dict(name=name, mesh=len(doc['meshes'])-1))
        doc['nodes'][assembly_nodes[assembly]]['children'].append(len(doc['nodes'])-1)
        points.extend(pos)
        triangles += len(indices)//3
    # Disabled variants omit the membrane and field entirely.
    if 'wormhole' in assembly_nodes and 'containment-field' in assembly_nodes:
        times = accessor([0., .9, 1.8, 2.7], 5126, 'SCALAR', bounds=True)
        scales = accessor([[1, 1, 1], [.76, 1, .76], [.36, 1, .36], [0, 1, 0]], 5126, 'VEC3')
        field = accessor([[1, 1, 1], [.80, .80, .80], [.28, .28, .28], [0, 0, 0]], 5126, 'VEC3')
        doc['animations'] = [dict(name='WormholeCollapse', samplers=[
            {'input': times, 'output': scales, 'interpolation': 'LINEAR'},
            {'input': times, 'output': field, 'interpolation': 'LINEAR'}], channels=[
            {'sampler': 0, 'target': {'node': assembly_nodes['wormhole'], 'path': 'scale'}},
            {'sampler': 1, 'target': {'node': assembly_nodes['containment-field'], 'path': 'scale'}}])]
    if info_overrides and info_overrides.get('state'):
        doc['nodes'][0]['name'] = info_overrides['name']
        doc['nodes'][0]['extras']['state'] = info_overrides['state']
    doc['buffers'][0]['byteLength'] = len(binary)
    encoded = json.dumps(doc, separators=(',', ':')).encode()
    encoded += b' ' * (-len(encoded) % 4)
    binary.extend(b'\0' * (-len(binary) % 4))
    result = struct.pack('<4sII', b'glTF', 2, 28+len(encoded)+len(binary))
    result += struct.pack('<I4s', len(encoded), b'JSON')+encoded
    result += struct.pack('<I4s', len(binary), b'BIN\0')+binary
    (ROOT/f'{file_stem}.glb').write_bytes(result)
    info = dict(name='Authority Nexus — Wormhole Anchor', triangles=triangles,
                renderSections=len(grouped), sourceComponents=len(parts),
                materials=len(materials), dimensionsXYZ=np.ptp(np.asarray(points), axis=0).round(4).tolist(),
                embeddedTextures=len(TEXTURES), textureResolution=[1536, 1024],
                towerTextureResolution=list(Image.open(ROOT/'textures'/TEXTURES[3]).size),
                ringTextureResolution=list(Image.open(ROOT/'textures'/TEXTURES[5]).size),
                powerTextureResolution=list(Image.open(ROOT/'textures'/TEXTURES[7]).size),
                assemblies=list(assembly_nodes), animations=[a['name'] for a in doc.get('animations', [])],
                axes='Y up, -Z forward', physicalDiameter='Unspecified; authored at approximately 20 relative units',
                generator='Python/NumPy; no Blender',
                texturing='Reference-projected hull; locally unwrapped armor, radiator edges, port caps and crowns; textured graphite structure, brass fittings and collars; blue reactor cells with selective emission; derived PBR masks',
                optimization='Static components merged by assembly and material; exact vertex deduplication; 16-bit indices',
                note='Concept reconstruction. Collapse animation affects the portal/field; hull destruction is not authored.')
    info.update(info_overrides or {})
    (ROOT/'model-info.json').write_text(json.dumps(info, indent=2)+'\n')

    lines = ['# Authority Nexus; Y up, -Z forward', f'mtllib {file_stem}.mtl']
    offset = 1
    for part in parts:
        lines.append('o '+part['name'])
        for mat, g in part['groups'].items():
            lines.append('usemtl material_'+str(mat))
            for x, y, z in g['position']:
                lines.append(f'v {x:.6f} {z:.6f} {-y:.6f}')
            for u, v in g['uv']:
                lines.append(f'vt {u:.7f} {1-v:.7f}')
            for x, y, z in g['normal']:
                lines.append(f'vn {x:.7f} {z:.7f} {-y:.7f}')
            idx = g['indices']
            for i in range(0, len(idx), 3):
                lines.append('f '+' '.join(f'{k+offset}/{k+offset}/{k+offset}' for k in idx[i:i+3]))
            offset += len(g['position'])
    (ROOT/f'{file_stem}.obj').write_text('\n'.join(lines)+'\n')
    mtl = []
    for i, mat in enumerate(materials):
        pbr = mat['pbrMetallicRoughness']
        mtl.extend([f'newmtl material_{i}', 'Kd '+' '.join(map(str, pbr['baseColorFactor'][:3])),
                    'Ks .3 .3 .3', 'Ns 48'])
        if 'baseColorTexture' in pbr:
            mtl.append('map_Kd textures/'+TEXTURES[pbr['baseColorTexture']['index']])
        if 'emissiveTexture' in mat:
            mtl.append('map_Ke textures/'+TEXTURES[mat['emissiveTexture']['index']])
        if 'emissiveFactor' in mat:
            mtl.append('Ke '+' '.join(map(str, mat['emissiveFactor'])))
        mtl.append('')
    (ROOT/f'{file_stem}.mtl').write_text('\n'.join(mtl))
    print(json.dumps(info, indent=2), flush=True)


if __name__ == '__main__':
    ROOT.mkdir(exist_ok=True)
    texture_files()
    build_geometry()
    export()
