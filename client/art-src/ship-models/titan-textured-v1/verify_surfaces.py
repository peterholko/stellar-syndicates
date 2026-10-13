"""Validate exported Titan geometry, articulated assemblies, and belly coverage."""
from collections import Counter
from pathlib import Path
import json
import struct
import numpy as np

ROOT = Path(__file__).resolve().parent
raw = (ROOT / 'titan.glb').read_bytes()
assert raw[:4] == b'glTF'
size = struct.unpack_from('<I', raw, 12)[0]
doc = json.loads(raw[20:20 + size])
binary = raw[28 + size:]


def read(index):
    a = doc['accessors'][index]
    view = doc['bufferViews'][a['bufferView']]
    width = {'VEC3': 3, 'VEC2': 2, 'SCALAR': 1}[a['type']]
    dtype = {5126: '<f4', 5125: '<u4', 5123: '<u2'}[a['componentType']]
    return np.frombuffer(binary, dtype=dtype, count=a['count'] * width,
                         offset=view.get('byteOffset', 0) + a.get('byteOffset', 0)).reshape(a['count'], width)


expected = {'hull | continuous armored pressure body', 'deck | central foredeck armor',
            'deck | aft central armor terrace', 'keel | deep segmented ventral armor',
            'bridge | recessed command blister', 'stern | central service fairing',
            'stern | dorsal service access'}
for side in ('port', 'starboard'):
    expected.update(f'armor.{side} | {part}' for part in
                    ('forward shelf', 'broad mid shoulder', 'aft stepped shoulder', 'lower shoulder flange'))
    expected.update(f'keel.{side}.{i} | ventral maintenance armor' for i in range(1, 5))
    expected.update(f'engineblock.{side} | {part}' for part in
                    ('hollow armored housing', 'dorsal armor lid', 'ventral service plate'))
report = {'triangles': 0, 'renderSections': 0, 'zeroAreaGeometry': 0,
          'zeroAreaTexturedUVs': 0, 'nonFiniteValues': 0, 'bottomComponents': {},
          'embeddedImagesMatch': True}
closed_geometry = {}
apertures = {}
assemblies = Counter()
for mesh in doc['meshes']:
    assemblies[mesh['extras']['assembly']] += 1
    for prim in mesh['primitives']:
        report['renderSections'] += 1
        positions = read(prim['attributes']['POSITION'])
        normals = read(prim['attributes']['NORMAL'])
        uv = read(prim['attributes']['TEXCOORD_0'])
        indices = read(prim['indices']).reshape(-1, 3)
        tri, tex = positions[indices], uv[indices]
        report['triangles'] += len(tri)
        report['nonFiniteValues'] += int(sum(np.count_nonzero(~np.isfinite(a)) for a in (positions, normals, uv)))
        cross = np.cross(tri[:, 1] - tri[:, 0], tri[:, 2] - tri[:, 0])
        area = np.linalg.norm(cross, axis=1)
        report['zeroAreaGeometry'] += int(np.count_nonzero(area < 1e-9))
        normal = cross / np.maximum(area[:, None], 1e-20)
        a, b = tex[:, 1] - tex[:, 0], tex[:, 2] - tex[:, 0]
        uv_area = np.abs(a[:, 0] * b[:, 1] - a[:, 1] * b[:, 0])
        material = doc['materials'][prim['material']]
        textured = 'baseColorTexture' in material['pbrMetallicRoughness']
        if textured:
            report['zeroAreaTexturedUVs'] += int(np.count_nonzero(uv_area < 1e-12))
        for component in mesh['extras']['sourceComponents']:
            name = component['name']
            start = component['indexStart'] // 3
            end = start + component['indexCount'] // 3
            if name == 'hull | continuous armored pressure body' or name.endswith(' | hollow armored housing'):
                closed_geometry.setdefault(name, []).extend(tri[start:end])
            if name.endswith(' | recessed textured plasma aperture'):
                apertures.setdefault(name, []).extend(tri[start:end])
            if name not in expected:
                continue
            selected = np.arange(start, end)[normal[start:end, 1] < -.35]
            if not len(selected):
                continue
            entry = report['bottomComponents'].setdefault(name, {'triangles': 0, 'textured': 0, 'bottomViewUVs': 0})
            entry['triangles'] += len(selected)
            if textured:
                entry['textured'] += len(selected)
                px = tex[selected] * 2048
                inside = (px[:, :, 0] >= 477) & (px[:, :, 0] <= 845) & (px[:, :, 1] >= 43) & (px[:, :, 1] <= 800)
                entry['bottomViewUVs'] += int(np.count_nonzero(inside.all(axis=1) & (uv_area[selected] > 1e-12)))
for img in doc['images']:
    view = doc['bufferViews'][img['bufferView']]
    embedded = binary[view['byteOffset']:view['byteOffset'] + view['byteLength']]
    report['embeddedImagesMatch'] &= embedded == (ROOT / 'textures' / img['name']).read_bytes()
report['missingUndersideComponents'] = sorted(expected - set(report['bottomComponents']))
report['fullyTexturedUnderside'] = not report['missingUndersideComponents'] and all(
    v['triangles'] == v['textured'] == v['bottomViewUVs'] for v in report['bottomComponents'].values())
closed = {}
for name, rows in closed_geometry.items():
    triangles = np.array(rows)
    edges = Counter()
    for triangle in triangles:
        points = [tuple(np.round(p, 6)) for p in triangle]
        for a, b in zip(points, points[1:] + points[:1]):
            edges[tuple(sorted((a, b)))] += 1
    volume = float(np.sum(np.einsum('ij,ij->i', triangles[:, 0], np.cross(triangles[:, 1], triangles[:, 2]))) / 6)
    closed[name] = {'closedManifold': all(n == 2 for n in edges.values()), 'signedVolume': volume}
report['closedHullAndEngineBlocks'] = closed
report['threeSolidPrimaryBodies'] = len(closed) == 3 and all(v['closedManifold'] and v['signedVolume'] > 0 for v in closed.values())
engine_positions = np.array([n['translation'] for n in doc['nodes'] if n['name'].startswith('socket.engine.')])
report['fourEnginesAcrossStern'] = len(engine_positions) == 4 and len(set(engine_positions[:, 0])) == 4 and float(np.ptp(engine_positions[:, 1])) < 1e-8 and float(np.ptp(engine_positions[:, 2])) < 1e-8
report['fourTallRecessedApertures'] = len(apertures) == 4 and all(np.ptp(np.array(rows).reshape(-1, 3), axis=0)[1] / np.ptp(np.array(rows).reshape(-1, 3), axis=0)[0] > 2 for rows in apertures.values())
report['assemblies'] = dict(assemblies)
report['fourIndependentTurretAssemblies'] = len(assemblies) == 5 and all(assemblies[f'turret.{side}.{i}'] == 4 for side in ('port', 'starboard') for i in (1, 2))
report['eightWeaponSockets'] = sum(n['name'].startswith('socket.weapon.') for n in doc['nodes']) == 8
report['passed'] = not any(report[k] for k in ('zeroAreaGeometry', 'zeroAreaTexturedUVs', 'nonFiniteValues')) and all(report[k] for k in ('fullyTexturedUnderside', 'embeddedImagesMatch', 'threeSolidPrimaryBodies', 'fourEnginesAcrossStern', 'fourTallRecessedApertures', 'fourIndependentTurretAssemblies', 'eightWeaponSockets'))
(ROOT / 'surface-verification.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({k: v for k, v in report.items() if k != 'bottomComponents'}, indent=2))
if not report['passed']:
    raise SystemExit(1)
