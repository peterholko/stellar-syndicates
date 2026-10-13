"""Render the actual traffic geometry/UVs and instance poses exported by tests.

This independently checks apparent scale and occlusion. The portal uses its
static export material; transparent wake blending is approximated as solid.
"""
from pathlib import Path
from io import BytesIO
import json
import struct
import numpy as np
from PIL import Image
from render_core import Model, render

ROOT = Path(__file__).resolve().parent


def main():
    model = Model(ROOT/'authority-nexus.glb')
    snapshot = json.loads((ROOT/'previews/traffic-snapshot.json').read_text())
    cache = {}

    def texture(ref):
        key = (ref['sourceFile'], ref['textureIndex'])
        if key not in cache:
            source = Path(ref['sourceFile'])
            if not source.exists():
                source = ROOT/'traffic-models'/source.name
            data = source.read_bytes()
            length = struct.unpack_from('<I', data, 12)[0]
            doc = json.loads(data[20:20+length])
            image = doc['images'][doc['textures'][ref['textureIndex']]['source']]
            view = doc['bufferViews'][image['bufferView']]
            start = 28+length+view.get('byteOffset', 0)
            png = data[start:start+view['byteLength']]
            model.images.append(np.asarray(Image.open(BytesIO(png)).convert('RGB'), dtype=np.float32)/255)
            model.doc['textures'].append({'source': len(model.images)-1})
            cache[key] = len(model.doc['textures'])-1
        return {'index': cache[key]}

    for part in snapshot['primitives']:
        source = part['material']
        mat = {'pbrMetallicRoughness': {'baseColorFactor': [*source['color'], 1]},
               'emissiveFactor': source['emissive'], 'doubleSided': source['doubleSided']}
        if source.get('map'):
            mat['pbrMetallicRoughness']['baseColorTexture'] = texture(source['map'])
        if source.get('emissiveMap'):
            mat['emissiveTexture'] = texture(source['emissiveMap'])
        if source['unlit']:
            mat['extensions'] = {'KHR_materials_unlit': {}}
        model.primitives.append({'name': part['name'], 'm': mat,
            'p': np.asarray(part['position']).reshape(-1, 3),
            'n': np.asarray(part['normal']).reshape(-1, 3),
            'uv': np.asarray(part['uv']).reshape(-1, 2),
            'i': np.asarray(part['indices']).reshape(-1, 3), 'components': []})
    # Keep the same framing as the station so the tiny ship scale is honest.
    render(model, (-.8, 1.42, -1.34), (0, 1, 0), 1400, 1050).save(ROOT/'previews/traffic-hero.png')
    render(model, (0, 1, 0), (0, 0, -1), 1400, 1050).save(ROOT/'previews/traffic-top.png')
    print('Rendered actual instanced ships at station scale: traffic-hero.png and traffic-top.png')


if __name__ == '__main__':
    main()
