"""Inspect the exported disabled-state solids and render actual mesh views."""
from pathlib import Path
from collections import defaultdict, Counter
import json
import numpy as np
from PIL import Image, ImageDraw, ImageFont
from render_core import Model, render
from inspect_model import VIEWS

ROOT = Path(__file__).resolve().parent/'disabled'
model = Model(ROOT/'authority-nexus-disabled.glb')
assert not model.doc.get('animations'), 'Disabled export must not reopen itself'
assert model.doc['nodes'][0]['extras']['state'] == 'attack-disabled'
components = defaultdict(list)
emergency = set()
triangles = 0
for p in model.primitives:
    assert not p['name'].startswith(('wormhole', 'containment-field'))
    assert all(np.isfinite(p[k]).all() for k in ('p', 'n', 'uv'))
    assert p['i'].min() >= 0 and p['i'].max() < len(p['p'])
    assert np.abs(np.linalg.norm(p['n'], axis=1)-1).max() < 1e-4
    vertices = p['p'][p['i']]
    cross = np.cross(vertices[:, 1]-vertices[:, 0], vertices[:, 2]-vertices[:, 0])
    assert np.linalg.norm(cross, axis=1).min() > 1e-10
    assert (cross*p['n'][p['i']].mean(axis=1)).sum(axis=1).min() > -1e-6
    assert np.linalg.norm(p['p'][:, [0, 2]], axis=1).min() > 3.6
    uv = p['uv']
    assert uv[:, 1].min() >= 0 and uv[:, 1].max() <= 1
    assert uv[:, 0].min() >= -1e-6 and uv[:, 0].max() <= 8.000001
    emissive = max(p['m'].get('emissiveFactor', [0, 0, 0])) > 0
    if emissive:
        assert p['m']['name'] == 'Emergency amber markers / reserve power'
    if p['name'].startswith('stabilizer-2 /'):
        assert p['p'][:, 1].max() < 2.7, 'Struck tower upper section still present'
    for c in p['components']:
        assert c['closed']
        start, end = c['indexStart']//3, (c['indexStart']+c['indexCount'])//3
        components[c['name']].extend(vertices[start:end].tolist())
        if emissive:
            emergency.add(c['name'])
    triangles += len(p['i'])
for name, data in components.items():
    edges = Counter()
    for tri in data:
        points = [tuple(np.round(v, 5)) for v in tri]
        for a, b in zip(points, points[1:]+points[:1]):
            edges[tuple(sorted([a, b]))] += 1
    assert all(count == 2 for count in edges.values()), (name, 'open solid')
    t = np.asarray(data)
    assert (t[:, 0]*np.cross(t[:, 1], t[:, 2])).sum()/6 > 1e-9, (name, 'negative volume')
assert len(emergency) == 7
assert sum('scorched-crater' in name for name in components) == 2
assert all(f'stabilizer-2.fractured-{side}-shell' in components for side in ('left', 'right'))
report = dict(triangles=triangles, closedSolidComponents=len(components), portalAbsent=True,
              containmentFieldAbsent=True, reactorEmissionZero=True, emergencyMarkers=len(emergency),
              fracturedStabilizer=2, impactCraters=2, finiteAttributes=True, consistentWinding=True,
              centralOpeningClear=True, noReopeningAnimation=True)
(ROOT/'geometry-validation.json').write_text(json.dumps(report, indent=2)+'\n')
print(json.dumps(report), flush=True)
folder = ROOT/'previews'
folder.mkdir(exist_ok=True)
views = ['hero', 'bottom-quarter', 'top', 'front', 'back', 'side']
for view in views:
    render(model, *VIEWS[view], 1400, 1050).save(folder/f'{view}.png')
    print('Rendered disabled', view, flush=True)
sheet = Image.new('RGB', (2100, 1100), '#111b26')
draw = ImageDraw.Draw(sheet)
font = ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf', 25)
draw.text((24, 16), 'AUTHORITY NEXUS / ATTACK DISABLED', font=font, fill='#e2b977')
for i, view in enumerate(views):
    x, y = i%3*700, 58+i//3*520
    sheet.paste(Image.open(folder/f'{view}.png').resize((700, 485), Image.Resampling.LANCZOS), (x, y))
    draw.text((x+24, y+478), view.upper(), font=font, fill='#a8b6bf')
sheet.save(ROOT/'authority-nexus-disabled-model-sheet.png')
