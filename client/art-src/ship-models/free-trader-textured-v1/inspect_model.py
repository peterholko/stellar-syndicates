"""Inspect exported Free Trader buffers and render their actual textured surfaces."""
from pathlib import Path
import json
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT.parent / 'mining-fleet-v1'))
from render_core import Model, render

model = Model(ROOT / 'free-trader.glb')
checks = model.validate()
components = set()
minimum_uv_area = 1.0
downward = 0
for primitive in model.primitives:
    material = primitive['m']
    assert 'baseColorTexture' in material['pbrMetallicRoughness']
    assert 'metallicRoughnessTexture' in material['pbrMetallicRoughness']
    assert 'emissiveTexture' in material
    uv = primitive['uv'][primitive['i']]
    a, b = uv[:, 1] - uv[:, 0], uv[:, 2] - uv[:, 0]
    areas = np.abs(a[:, 0] * b[:, 1] - a[:, 1] * b[:, 0]) / 2
    assert areas.min() > 1e-12, (primitive['name'], 'collapsed UVs')
    minimum_uv_area = min(minimum_uv_area, float(areas.min()))
    downward += int(np.count_nonzero(primitive['n'][primitive['i']].mean(axis=1)[:, 1] < -.55))
    components.update(c['name'] for c in primitive['components'])
counts = {
    'canopyPanels': sum(n.startswith('canopy.panel.') for n in components),
    'dockingCollars': sum(n.startswith('docking.') and n.endswith('.housing') for n in components),
    'mainEngines': sum(n.startswith('engine.') and n.endswith('.housing') for n in components),
    'bridges': sum(n == 'bridge.glazing' for n in components),
    'guns': sum(n.startswith(('weapon.', 'turret.', 'gun.')) for n in components),
}
assert counts == dict(canopyPanels=8, dockingCollars=4, mainEngines=2, bridges=1, guns=0), counts
sockets = {n['name']: n for n in model.doc['nodes'] if n['name'].startswith('socket.')}
expected = {**{f'socket.docking.{i}': [-1., 0., 0.] if i < 3 else [1., 0., 0.] for i in range(1, 5)},
            'socket.engine.port': [0., 0., 1.], 'socket.engine.starboard': [0., 0., 1.]}
assert set(sockets) == set(expected)
for name, direction in expected.items():
    assert np.allclose(sockets[name]['extras']['direction'], direction), name
    assert np.isfinite(sockets[name]['translation']).all(), name
assert 6000 <= checks['triangles'] <= 8000
assert len(model.primitives) == 2
assert all(image.shape == (2048, 2048, 3) for image in model.images)
color, _, emissive = model.images
r, g, b = color.transpose(2, 0, 1)
purple = (b > r * 1.18) & (r > g * 1.15) & (b > 85/255)
assert np.count_nonzero(purple) > 1000
assert not np.any(emissive[purple]), 'Purple paint must not emit light'
checks.update(observedFeatureCounts=counts, allPhysicalTrianglesTextured=True,
              allPhysicalTrianglesHaveAreaUVs=True, minimumUVTriangleArea=minimum_uv_area,
              texturedDownwardTriangles=downward, socketNodes=list(sockets.values()),
              purplePaintIsNonEmissive=True)
(ROOT / 'surface-verification.json').write_text(json.dumps(checks, indent=2) + '\n')
preview = ROOT / 'previews'
preview.mkdir(exist_ok=True)
views = [('three-quarter', (1.2, .95, -1.45), (0, 1, 0)),
         ('underside', (1, -1, -1.3), (0, 1, 0)),
         ('top', (0, 1, 0), (0, 0, -1)), ('bottom', (0, -1, 0), (0, 0, -1)),
         ('front', (0, 0, -1), (0, 1, 0)), ('back', (0, 0, 1), (0, 1, 0)),
         ('side', (1, 0, 0), (0, 1, 0))]
for name, camera, up in views:
    print('Rendering', name, flush=True)
    render(model, camera, up, width=1000, height=850).save(preview / (name + '.png'))
sheet = Image.new('RGB', (1800, 1200), (12, 16, 23))
draw = ImageDraw.Draw(sheet)
try:
    font = ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf', 22)
except OSError:
    font = ImageFont.load_default()
for i, (name, _, _) in enumerate(views[:6]):
    x, y = (i % 3) * 600, (i // 3) * 600
    im = Image.open(preview / (name + '.png'))
    im.thumbnail((580, 530))
    sheet.paste(im, (x + (600 - im.width) // 2, y + 12))
    draw.text((x + 24, y + 555), name.upper(), font=font, fill=(190, 202, 213))
sheet.save(preview / 'contact-sheet.png')
print(json.dumps({k: checks[k] for k in ('triangles', 'closedComponents', 'texturedDownwardTriangles', 'observedFeatureCounts')}, indent=2))
