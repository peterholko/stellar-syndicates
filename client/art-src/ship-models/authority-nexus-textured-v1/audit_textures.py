"""Check texture coverage on every exported face, including hidden solid caps."""
from pathlib import Path
from collections import defaultdict
from hashlib import sha256
import argparse
import copy
import json
import numpy as np
from PIL import Image, ImageDraw, ImageFont
from render_core import Model, bilinear, render

ROOT = Path(__file__).resolve().parent
FILES = {'operational': 'authority-nexus.glb', 'disabled': 'disabled/authority-nexus-disabled.glb'}
UNIFORM = {'Powered cyan stabilizer field': 'Free containment energy',
           'Emergency amber markers / reserve power': 'Small uniform emergency lamps'}


def audit(state, model, baseline):
    parts = defaultdict(list)
    samples = defaultdict(list)
    totals = defaultdict(int)
    exceptions = defaultdict(int)
    materials = {}
    minimum_area = float('inf')
    # Interior samples supplement the UV-area test: a mapped face must sample
    # actual surface detail rather than only a single constant texel.
    bary = np.array([[1/3, 1/3, 1/3], [.7, .15, .15], [.15, .7, .15], [.15, .15, .7]])
    for p in model.primitives:
        material = p['m']
        name = material['name']
        texture_info = material['pbrMetallicRoughness'].get('baseColorTexture')
        intentional_uniform = name in UNIFORM
        is_portal = 'Violet wormhole energy' in name
        assert texture_info or intentional_uniform, f'{state}: untextured material {name}'
        totals['triangles'] += len(p['i'])
        if intentional_uniform:
            exceptions[UNIFORM[name]] += len(p['i'])
        elif not is_portal:
            totals['physicalTriangles'] += len(p['i'])
        colors = None
        if texture_info:
            uv = p['uv'][p['i']]
            assert np.isfinite(uv).all(), (state, name, 'Nonfinite UV')
            a, b = uv[:, 1]-uv[:, 0], uv[:, 2]-uv[:, 0]
            area = np.abs(a[:, 0]*b[:, 1]-a[:, 1]*b[:, 0])/2
            assert area.min() > 5e-11, (state, name, 'Collapsed texture mapping')
            minimum_area = min(minimum_area, float(area.min()))
            texture = model.doc['textures'][texture_info['index']]
            sampler = model.doc['samplers'][texture['sampler']]
            wrap_s, wrap_t = sampler.get('wrapS', 10497) == 10497, sampler.get('wrapT', 10497) == 10497
            for axis, repeat in enumerate((wrap_s, wrap_t)):
                if not repeat:
                    assert uv[:, :, axis].min() >= -1e-6 and uv[:, :, axis].max() <= 1.000001
            image = model.images[texture['source']]
            assert image.shape[0] > 1 and image.shape[1] > 1
            sample_uv = np.einsum('bv,tvc->tbc', bary, uv)
            colors = bilinear(image, sample_uv.reshape(-1, 2), wrap_s, wrap_t).reshape(-1, len(bary), 3)
            totals['texturedTriangles'] += len(p['i'])
            materials[name] = model.doc['images'][texture['source']]['name']
        for c in p['components']:
            start, end = c['indexStart']//3, (c['indexStart']+c['indexCount'])//3
            if intentional_uniform:
                assert c['name'].startswith(('containment.field-rim', 'emergency.inner-marker-', 'stabilizer-'))
                if state == 'disabled':
                    assert c['name'].startswith('emergency.inner-marker-') or c['name'].endswith('.beacon')
                else:
                    assert c['name'] == 'containment.field-rim' or '.field-tether.' in c['name']
            if colors is not None:
                samples[c['name']].extend(colors[start:end].reshape(-1, 3).tolist())
            for tri in p['i'][start:end]:
                parts[c['name']].append(p['p'][tri].astype('<f4').tobytes()+p['n'][tri].astype('<f4').tobytes())
    digest = sha256()
    for name, triangles in sorted(parts.items()):
        digest.update(name.encode())
        for triangle in sorted(triangles):
            digest.update(triangle)
    assert digest.hexdigest() == baseline['geometryNormalsSha256'], f'{state}: geometry or normals changed'
    variation = {name: float(np.max(np.ptp(values, axis=0))) for name, values in samples.items()}
    assert min(variation.values()) > 1/255, 'A component samples a featureless portion of its texture'
    return dict(model=FILES[state], modelSha256=sha256((ROOT/FILES[state]).read_bytes()).hexdigest(),
                **totals, texturedComponents=len(samples), untexturedPhysicalTriangles=0,
                collapsedTexturedTriangles=0, minimumUvArea=minimum_area,
                sampledTextureVariationOnEveryTexturedComponent=True,
                minimumComponentColorRange=min(variation.values()),
                intentionalUniformSurfaces=dict(exceptions), materialTextures=materials,
                geometryAndNormalsUnchanged=True, geometryNormalsSha256=digest.hexdigest(),
                renderSections=len(model.primitives))


def detail(model, match, camera):
    selected = copy.copy(model)
    selected.primitives = []
    for p in model.primitives:
        triangles = []
        for c in p['components']:
            if match(c['name']):
                start, end = c['indexStart']//3, (c['indexStart']+c['indexCount'])//3
                triangles.extend(p['i'][start:end])
        if triangles:
            selected.primitives.append({**p, 'i': np.asarray(triangles)})
    selected.positions = np.concatenate([p['p'][np.unique(p['i'])] for p in selected.primitives])
    return render(selected, camera, (0, 1, 0), 900, 650)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--render', action='store_true')
    args = parser.parse_args()
    before = json.loads((ROOT/'texture-audit-before.json').read_text())
    models = {state: Model(ROOT/path) for state, path in FILES.items()}
    reports = {state: audit(state, model, before[state]) for state, model in models.items()}
    result = dict(method='Every exported GLB triangle: embedded albedo, finite noncollapsed UVs, sampler bounds, interior color samples and original per-component geometry/normal digest.',
                  models=reports)
    (ROOT/'texture-validation.json').write_text(json.dumps(result, indent=2)+'\n')
    for state, report in reports.items():
        print(state, json.dumps({k: v for k, v in report.items() if k not in ('materialTextures', 'modelSha256', 'geometryNormalsSha256')}), flush=True)
    if args.render:
        views = [
            ('OPERATIONAL / UNDERSIDE RIB, PIPE AND RADIATOR', 'operational',
             lambda n: n in ('belly.rib.2', 'coolant.2', 'belly.radiator.2'), (1, -1.5, 1)),
            ('OPERATIONAL / TOWER CROWN AND COIL', 'operational',
             lambda n: n in ('stabilizer-1.crown', 'stabilizer-1.beacon', 'stabilizer-1.coil.5'), (1, .5, .8)),
            ('DISABLED / FRACTURED CORE AND EXPOSED CAP', 'disabled',
             lambda n: n in ('stabilizer-2.broken-powered-core', 'stabilizer-2.exposed-core-cap') or n.startswith('stabilizer-2.coil.'), (-1, 1, -.8)),
            ('DISABLED / EMITTER MOUNT, JAWS AND CONDUITS', 'disabled',
             lambda n: n.startswith(('stabilizer-1.emitter-mount', 'stabilizer-1.emitter-jaw.', 'stabilizer-1.conduit-housing.')),
             (1, 1.6, 1)),
        ]
        sheet = Image.new('RGB', (1800, 1400), '#101821')
        draw = ImageDraw.Draw(sheet)
        font = ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf', 20)
        for i, (label, state, match, camera) in enumerate(views):
            x, y = (i % 2)*900, (i//2)*700
            sheet.paste(detail(models[state], match, camera), (x, y+40))
            draw.text((x+22, y+15), label, font=font, fill='#c4cbd0')
        path = ROOT/'previews/texture-coverage-detail.png'
        sheet.save(path)
        print('Rendered', path, flush=True)


if __name__ == '__main__':
    main()
