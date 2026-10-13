"""Validate exported geometry and render real mesh previews, independently of WebGL."""
from pathlib import Path
from collections import Counter, defaultdict
import argparse
import json
import numpy as np
from PIL import Image, ImageDraw, ImageFont
from render_core import Model, render, bilinear

ROOT = Path(__file__).resolve().parent
VIEWS = {
    'hero': ((-.80, 1.42, -1.34), (0, 1, 0)),
    'rear-quarter': ((1.1, 1.35, 1.3), (0, 1, 0)),
    'bottom-quarter': ((1.1, -1.65, 1.3), (0, -1, 0)),
    'top': ((0, 1, 0), (0, 0, -1)),
    'bottom': ((0, -1, 0), (0, 0, -1)),
    'front': ((0, 0, -1), (0, 1, 0)),
    'back': ((0, 0, 1), (0, 1, 0)),
    'side': ((-1, 0, 0), (0, 1, 0)),
}


def validate(model):
    components = defaultdict(list)
    closed = {}
    total = 0
    ventral = 0
    minimum_r = []
    textured_tower_shells = set()
    textured_collar_triangles = 0
    textured_power = set()
    textured_coils = set()
    textured_couplings = set()
    for p in model.primitives:
        for key in ['p', 'n', 'uv']:
            assert np.isfinite(p[key]).all(), (p['name'], key)
        assert p['i'].min() >= 0 and p['i'].max() < len(p['p'])
        assert np.abs(np.linalg.norm(p['n'], axis=1)-1).max() < 1e-4
        vertices = p['p'][p['i']]
        cross = np.cross(vertices[:, 1]-vertices[:, 0], vertices[:, 2]-vertices[:, 0])
        area = np.linalg.norm(cross, axis=1)/2
        assert area.min() > 1e-10, (p['name'], 'degenerate triangle')
        assert (cross*p['n'][p['i']].mean(axis=1)).sum(axis=1).min() > -1e-6, (p['name'], 'normal winding')
        if p['m']['name'] in ('Machined brass containment collars', 'Brushed brass fittings'):
            assert p['uv'][:, 0].min() >= -1e-6 and p['uv'][:, 0].max() <= 8.000001
            assert p['uv'][:, 1].min() >= 0 and p['uv'][:, 1].max() <= 1
        else:
            assert p['uv'].min() >= 0 and p['uv'].max() <= 1
        if p['m']['name'].startswith('Ventral'):
            ventral += len(p['i'])
        if not p['name'].startswith(('wormhole', 'containment-field')):
            minimum_r.append(np.linalg.norm(p['p'][:, [0, 2]], axis=1).min())
        for c in p['components']:
            start = c['indexStart']//3
            end = (c['indexStart']+c['indexCount'])//3
            components[c['name']].extend(vertices[start:end].tolist())
            closed[c['name']] = c['closed']
            if c['name'] == 'hull.ventral-collar':
                ventral += end-start
                pbr = p['m']['pbrMetallicRoughness']
                texture = model.doc['textures'][pbr['baseColorTexture']['index']]
                assert model.doc['images'][texture['source']]['name'] == 'authority-nexus-brass-ring.png'
                assert model.doc['samplers'][texture['sampler']]['wrapS'] == 10497
                assert 'metallicRoughnessTexture' in pbr
                uv = p['uv'][p['i'][start:end]]
                a, b = uv[:, 1]-uv[:, 0], uv[:, 2]-uv[:, 0]
                assert np.abs(a[:, 0]*b[:, 1]-a[:, 1]*b[:, 0]).min() > 1e-8, 'Collapsed collar UVs'
                assert np.ptp(uv[:, :, 0], axis=1).max() < .251, 'Stretched collar texture at angular seam'
                colors = bilinear(model.images[texture['source']], uv.mean(axis=1), wrap_s=True)
                assert np.ptp(colors.mean(axis=1)) > .10, 'Collar material lacks surface variation'
                textured_collar_triangles += end-start
            is_core = c['name'].endswith('.powered-core')
            is_feed = '.power-feed.' in c['name'] or c['name'].endswith('.beacon')
            is_coil = '.coil.' in c['name']
            is_coupling = '.coupling.' in c['name']
            if is_core or is_feed or is_coil or is_coupling:
                pbr = p['m']['pbrMetallicRoughness']
                texture = model.doc['textures'][pbr['baseColorTexture']['index']]
                expected = 'authority-nexus-power-cells.png' if is_core or is_feed else 'authority-nexus-brass-ring.png'
                assert model.doc['images'][texture['source']]['name'] == expected
                assert model.doc['samplers'][texture['sampler']]['wrapS'] == 10497
                assert 'metallicRoughnessTexture' in pbr
                uv = p['uv'][p['i'][start:end]]
                a, b = uv[:, 1]-uv[:, 0], uv[:, 2]-uv[:, 0]
                assert np.abs(a[:, 0]*b[:, 1]-a[:, 1]*b[:, 0]).min() > 1e-8, (c['name'], 'collapsed cylinder UVs')
                segments = 12 if is_core else 6 if is_feed else 8
                walls = uv[segments*2-4:]
                seam_limit = .168 if is_core or is_feed else .376
                assert np.ptp(walls[:, :, 0], axis=1).max() < seam_limit, (c['name'], 'stretched cylindrical seam')
                colors = bilinear(model.images[texture['source']], uv.mean(axis=1), wrap_s=True)
                assert np.ptp(colors.mean(axis=1)) > .025, (c['name'], 'missing texture variation')
                if is_core or is_feed:
                    em = model.doc['textures'][p['m']['emissiveTexture']['index']]
                    assert model.doc['images'][em['source']]['name'] == 'authority-nexus-power-cells-emissive.png'
                    emission = bilinear(model.images[em['source']], uv.mean(axis=1), wrap_s=True)
                    assert emission.max() > .1 and np.ptp(emission.mean(axis=1)) > .025
                    textured_power.add(c['name'])
                elif is_coil:
                    textured_coils.add(c['name'])
                else:
                    textured_couplings.add(c['name'])
            if c['name'].endswith(('.left-shell', '.right-shell')):
                pbr = p['m']['pbrMetallicRoughness']
                texture = model.doc['textures'][pbr['baseColorTexture']['index']]
                assert model.doc['images'][texture['source']]['name'] == 'authority-nexus-ivory-panels.png'
                assert 'metallicRoughnessTexture' in pbr
                uv = p['uv'][p['i'][start:end]]
                a, b = uv[:, 1]-uv[:, 0], uv[:, 2]-uv[:, 0]
                assert np.abs(a[:, 0]*b[:, 1]-a[:, 1]*b[:, 0]).min() > 1e-9, (c['name'], 'collapsed tower UVs')
                colors = bilinear(model.images[texture['source']], uv.mean(axis=1))
                assert np.ptp(colors.mean(axis=1)) > .10, (c['name'], 'tower texture lacks visible detail')
                textured_tower_shells.add(c['name'])
        total += len(p['i'])
    count = 0
    for name, triangles in components.items():
        edges = Counter()
        for triangle in triangles:
            points = [tuple(np.round(p, 5)) for p in triangle]
            for a, b in zip(points, points[1:]+points[:1]):
                edges[tuple(sorted([a, b]))] += 1
        if closed[name]:
            assert all(n == 2 for n in edges.values()), (name, 'open/non-manifold solid', Counter(edges.values()))
            t = np.asarray(triangles)
            volume = (t[:, 0]*np.cross(t[:, 1], t[:, 2])).sum()/6
            assert volume > 1e-9, (name, 'non-positive signed volume', volume)
            count += 1
        else:
            assert name == 'wormhole.energy-membrane'
            assert all(n in (1, 2) for n in edges.values())
            assert sum(n == 1 for n in edges.values()) == 64
    assert min(minimum_r) > 3.6, 'Unexpected hardware blocking portal center'
    assert ventral > 200
    assert textured_collar_triangles == 256
    assert len(textured_power) == 40 and sum(name.endswith('.powered-core') for name in textured_power) == 4
    assert len(textured_coils) == 24 and len(textured_couplings) == 32
    assert textured_tower_shells == {f'stabilizer-{i}.{side}-shell'
                                    for i in range(1, 5) for side in ['left', 'right']}
    assemblies = [n['name'] for n in model.doc['nodes'] if n.get('extras', {}).get('assembly')]
    assert all(f'stabilizer-{i}' in assemblies for i in range(1, 5))
    assert 'wormhole' in assemblies and 'containment-field' in assemblies
    animation = model.doc['animations'][0]
    assert animation['name'] == 'WormholeCollapse'
    values = model.accessor(animation['samplers'][0]['output'])
    assert np.allclose(values[0], [1, 1, 1]) and np.allclose(values[-1], [0, 1, 0])
    return dict(triangles=total, closedSolidComponents=count,
                intentionalOpenEnergySurfaces=1, dedicatedUndersideTriangles=ventral,
                finiteAttributes=True, consistentWinding=True, nondegenerateTriangles=True,
                boundedUVs=True, fourSeparateStabilizers=True, portalCenterClear=True,
                texturedTowerShells=len(textured_tower_shells), noncollapsedTowerUVs=True,
                texturedVentralCollarTriangles=textured_collar_triangles, repeatingCollarUVsVerified=True,
                texturedPowerCylinders=len(textured_power), texturedReactorRings=len(textured_coils),
                texturedFeedCouplings=len(textured_couplings), cylinderTextureUVsVerified=True,
                minimumHardwareRadius=round(float(min(minimum_r)), 4),
                collapseAnimationVerified=True,
                method='Independent analysis of exported GLB attributes and welded per-component topology')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--quick', action='store_true')
    args = parser.parse_args()
    model = Model(ROOT/'authority-nexus.glb')
    report = validate(model)
    (ROOT/'geometry-validation.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps({k: v for k, v in report.items() if k != 'method'}), flush=True)
    folder = ROOT/'previews'
    folder.mkdir(exist_ok=True)
    views = ['hero', 'bottom-quarter'] if args.quick else list(VIEWS)
    for view in views:
        camera, up = VIEWS[view]
        render(model, camera, up, 1400, 1050).save(folder/f'{view}.png')
        print('Rendered', view, flush=True)
    model.primitives = [p for p in model.primitives if not p['name'].startswith(('wormhole', 'containment-field'))]
    render(model, *VIEWS['hero'], 1400, 1050).save(folder/'portal-off.png')
    print('Rendered portal-off; actual mesh opening is exposed', flush=True)
    detail = Model(ROOT/'authority-nexus.glb')
    selected = []
    for p in detail.primitives:
        triangles = []
        for c in p['components']:
            if c['name'] == 'stabilizer-1.powered-core' or c['name'].startswith('stabilizer-1.coil.'):
                triangles.extend(p['i'][c['indexStart']//3:(c['indexStart']+c['indexCount'])//3])
        if triangles:
            selected.append({**p, 'i': np.asarray(triangles)})
    detail.primitives = selected
    detail.positions = np.concatenate([p['p'][np.unique(p['i'])] for p in selected])
    render(detail, (1, .22, .72), (0, 1, 0), 700, 1200).save(folder/'power-core-detail.png')
    print('Rendered power core and ring detail', flush=True)
    if not args.quick:
        sheet = Image.new('RGB', (2100, 1640), '#111b26')
        d = ImageDraw.Draw(sheet)
        font = ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf', 25)
        d.text((30, 19), 'AUTHORITY NEXUS / EXPORTED 3D MODEL', font=font, fill='#e5d8bb')
        for i, (view, label) in enumerate([
            ('hero', 'UPPER QUARTER'), ('bottom-quarter', 'UNDERSIDE QUARTER'), ('portal-off', 'PORTAL OFF'),
            ('top', 'TOP'), ('bottom', 'BOTTOM'), ('front', 'FRONT'),
            ('back', 'BACK'), ('side', 'SIDE'), ('rear-quarter', 'REAR QUARTER')]):
            x, y = (i % 3)*700, 65+(i//3)*520
            im = Image.open(folder/f'{view}.png').resize((700, 485), Image.Resampling.LANCZOS)
            sheet.paste(im, (x, y))
            d.text((x+23, y+477), label, font=font, fill='#9db0c0')
        sheet.save(ROOT/'authority-nexus-model-sheet.png')


if __name__ == '__main__':
    main()
