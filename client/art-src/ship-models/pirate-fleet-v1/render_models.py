"""Read exported GLBs and render their actual triangles/UVs in software.

Independent of the mesh builder and browser. This is an asset inspection tool,
not a WebGL/PBR compatibility test. Textures are sampled from embedded PNGs.
"""
from pathlib import Path
from io import BytesIO
import argparse
import json
import struct
import numpy as np
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent
SHIPS = ['privateer-raider', 'pirate-corsair', 'pirate-boarding-raider']
TITLES = ['Privateer Raider', 'Pirate Corsair', 'Pirate Boarding Raider']


def unit(v):
    v = np.asarray(v, float)
    return v/np.linalg.norm(v)


class Model:
    def __init__(self, filename):
        data = filename.read_bytes()
        magic, version, length = struct.unpack_from('<4sII', data)
        assert magic == b'glTF' and version == 2 and length == len(data)
        jlen = struct.unpack_from('<I', data, 12)[0]
        self.doc = json.loads(data[20:20+jlen])
        self.bin = data[28+jlen:]
        self.images = [np.asarray(Image.open(BytesIO(self.view(i['bufferView']))).convert('RGB'),
                                  dtype=np.float32)/255 for i in self.doc['images']]
        self.primitives = []
        for mesh in self.doc['meshes']:
            for primitive in mesh['primitives']:
                a = primitive['attributes']
                self.primitives.append({
                    'p': self.accessor(a['POSITION']), 'n': self.accessor(a['NORMAL']),
                    'uv': self.accessor(a['TEXCOORD_0']),
                    'i': self.accessor(primitive['indices']).reshape(-1, 3),
                    'm': self.doc['materials'][primitive['material']],
                    'name': mesh['name'], 'components': mesh['extras']['sourceComponents']})
        self.positions = np.concatenate([p['p'] for p in self.primitives])

    def view(self, index):
        view = self.doc['bufferViews'][index]
        start = view.get('byteOffset', 0)
        return self.bin[start:start+view['byteLength']]

    def accessor(self, index):
        a = self.doc['accessors'][index]
        n = {'SCALAR': 1, 'VEC2': 2, 'VEC3': 3}[a['type']]
        dtype = {5126: '<f4', 5125: '<u4', 5123: '<u2'}[a['componentType']]
        return np.frombuffer(self.view(a['bufferView']), dtype=dtype,
                             count=a['count']*n, offset=a.get('byteOffset', 0)).reshape(-1, n)

    def validate(self):
        """Index, normal, triangle, UV, underside, and closed-component checks."""
        from collections import Counter, defaultdict
        components = defaultdict(list)
        total, ventral = 0, 0
        signed_volumes = {}
        for p in self.primitives:
            assert all(np.isfinite(p[k]).all() for k in ['p', 'n', 'uv'])
            assert p['i'].max() < len(p['p'])
            assert np.abs(np.linalg.norm(p['n'], axis=1)-1).max() < 1e-4
            vertices = p['p'][p['i']]
            cross = np.cross(vertices[:, 1]-vertices[:, 0], vertices[:, 2]-vertices[:, 0])
            area = np.linalg.norm(cross, axis=1)/2
            assert area.min() > 1e-10, (p['name'], 'degenerate triangle')
            outward = (cross*p['n'][p['i']].mean(axis=1)).sum(axis=1)
            assert outward.min() > -1e-5, (p['name'], 'reversed normal')
            assert (p['uv'] >= 0).all() and (p['uv'] <= 1).all()
            uv = p['uv'][p['i']]
            # Dedicated ventral material means broad downward faces use the
            # bottom view; small solid-color structural braces are intentional.
            if p['m']['name'].startswith('Ventral'):
                a, b = uv[:, 1]-uv[:, 0], uv[:, 2]-uv[:, 0]
                uv_area = np.abs(a[:, 0]*b[:, 1]-a[:, 1]*b[:, 0])/2
                assert uv_area.min() > 1e-12, 'Collapsed underside UVs'
                ventral += len(area)
            for component in p['components']:
                start, end = component['indexStart']//3, (component['indexStart']+component['indexCount'])//3
                components[component['name']].extend(vertices[start:end].tolist())
            total += len(area)
        closed = []
        for name, triangles in components.items():
            edges = Counter()
            for triangle in triangles:
                vertices = [tuple(np.round(p, 5)) for p in triangle]
                for a, b in zip(vertices, vertices[1:]+vertices[:1]):
                    edges[tuple(sorted([a, b]))] += 1
            assert all(count == 2 for count in edges.values()), (name, 'non-manifold boundary')
            vertices = np.asarray(triangles)
            volume = (vertices[:, 0]*np.cross(vertices[:, 1], vertices[:, 2])).sum()/6
            assert volume > 1e-9, (name, 'non-positive volume')
            signed_volumes[name] = round(float(volume), 8)
            closed.append(name)
        assert ventral > 30
        return {'triangles': total, 'components': len(closed), 'closedComponents': len(closed),
                'dedicatedBottomTriangles': ventral, 'finiteAttributes': True,
                'unitNormals': True, 'consistentWinding': True, 'nondegenerateTriangles': True,
                'noncollapsedBottomUVs': True, 'positiveVolumes': signed_volumes,
                'method': 'Independent checks on exported GLB buffers; welded position topology per authored component'}


def bilinear(texture, uv):
    h, w = texture.shape[:2]
    xy = np.clip(uv, 0, 1)*np.array([w-1, h-1])
    a = np.floor(xy).astype(int)
    b = np.minimum(a+1, [w-1, h-1])
    mix = xy-a
    x, y = mix[:, 0:1], mix[:, 1:2]
    return ((texture[a[:, 1], a[:, 0]]*(1-x)+texture[a[:, 1], b[:, 0]]*x)*(1-y)+
            (texture[b[:, 1], a[:, 0]]*(1-x)+texture[b[:, 1], b[:, 0]]*x)*y)


def render(model, camera, up=(0, 1, 0), width=1100, height=850):
    direction = unit(camera)
    right = unit(np.cross(up, direction))
    vertical = unit(np.cross(direction, right))
    basis = np.column_stack([right, vertical, direction])
    transformed = model.positions@basis
    low, high = transformed.min(axis=0), transformed.max(axis=0)
    middle = (low+high)/2
    scale = min((width-100)/(high[0]-low[0]), (height-100)/(high[1]-low[1]))
    frame = np.empty((height, width, 3), dtype=np.float32)
    # A quiet charcoal studio background. No model material is hidden by bloom.
    yy, xx = np.mgrid[:height, :width]
    glow = np.exp(-((xx-width*.50)**2/(width*.62)**2+(yy-height*.45)**2/(height*.65)**2))
    frame[:] = np.array([.032, .043, .058])+glow[:, :, None]*np.array([.024, .030, .036])
    depth = np.full((height, width), -np.inf)
    key = unit(direction*.65+np.array([-.45, .95, -.35]))
    fill = unit(direction+np.array([.80, -.20, .25]))
    for primitive in model.primitives:
        projected = (primitive['p']@basis-middle)*scale
        projected[:, 0] += width/2
        projected[:, 1] = height/2-projected[:, 1]
        pbr = primitive['m']['pbrMetallicRoughness']
        basefactor = np.array(pbr.get('baseColorFactor', [1, 1, 1, 1])[:3])
        color_tex = model.images[model.doc['textures'][pbr['baseColorTexture']['index']]['source']] if 'baseColorTexture' in pbr else None
        em = primitive['m'].get('emissiveTexture')
        emissive_tex = model.images[model.doc['textures'][em['index']]['source']] if em else None
        em_factor = np.array(primitive['m'].get('emissiveFactor', [0, 0, 0]))
        for indices in primitive['i']:
            points = primitive['p'][indices]
            face = np.cross(points[1]-points[0], points[2]-points[0])
            if face@direction <= 1e-10:
                continue
            v = projected[indices]
            minx, miny = np.maximum(np.floor(v[:, :2].min(axis=0)).astype(int), [0, 0])
            maxx, maxy = np.minimum(np.ceil(v[:, :2].max(axis=0)).astype(int), [width-1, height-1])
            if maxx < minx or maxy < miny:
                continue
            y, x = np.mgrid[miny:maxy+1, minx:maxx+1]
            x, y = x+.5, y+.5
            denominator = (v[1, 1]-v[2, 1])*(v[0, 0]-v[2, 0])+(v[2, 0]-v[1, 0])*(v[0, 1]-v[2, 1])
            if abs(denominator) < 1e-9:
                continue
            a = ((v[1, 1]-v[2, 1])*(x-v[2, 0])+(v[2, 0]-v[1, 0])*(y-v[2, 1]))/denominator
            b = ((v[2, 1]-v[0, 1])*(x-v[2, 0])+(v[0, 0]-v[2, 0])*(y-v[2, 1]))/denominator
            c = 1-a-b
            d = a*v[0, 2]+b*v[1, 2]+c*v[2, 2]
            area_depth = depth[miny:maxy+1, minx:maxx+1]
            mask = (a >= -1e-7) & (b >= -1e-7) & (c >= -1e-7) & (d > area_depth)
            if not mask.any():
                continue
            bary = np.column_stack([a[mask], b[mask], c[mask]])
            uv = bary@primitive['uv'][indices]
            normals = bary@primitive['n'][indices]
            normals /= np.maximum(np.linalg.norm(normals, axis=1, keepdims=True), 1e-8)
            color = np.power(bilinear(color_tex, uv), 2.2)*basefactor if color_tex is not None else np.broadcast_to(basefactor, (len(uv), 3))
            light = .50+.38*np.clip(normals@key, 0, 1)+.18*np.clip(normals@fill, 0, 1)
            color = color*light[:, None]
            if emissive_tex is not None:
                color += np.power(bilinear(emissive_tex, uv), 2.2)*em_factor
            else:
                color += em_factor
            half = unit(key+direction)
            specular = np.power(np.clip(normals@half, 0, 1), 38)*.035
            color += specular[:, None]
            rgb = np.power(np.clip(color, 0, 1), 1/2.2)
            frame[miny:maxy+1, minx:maxx+1][mask] = rgb
            area_depth[mask] = d[mask]
    return Image.fromarray(np.round(np.clip(frame, 0, 1)*255).astype('uint8'))


VIEWS = {
    'hero': ((-1.10, 1.55, -1.45), (0, 1, 0)),
    'rear-quarter': ((-1.2, .92, 1.5), (0, 1, 0)),
    'bottom-quarter': ((1.1, -1.65, 1.4), (0, -1, 0)),
    'top': ((0, 1, 0), (0, 0, -1)),
    'bottom': ((0, -1, 0), (0, 0, -1)),
    'front': ((0, 0, -1), (0, 1, 0)),
    'back': ((0, 0, 1), (0, 1, 0)),
    'side': ((-1, 0, 0), (0, 1, 0)),
}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--quick', action='store_true')
    parser.add_argument('--ship', choices=SHIPS)
    args = parser.parse_args()
    for ship in ([args.ship] if args.ship else SHIPS):
        folder = ROOT.parent/f'{ship}-textured-v1'
        model = Model(folder/f'{ship}.glb')
        report = model.validate()
        (folder/'geometry-validation.json').write_text(json.dumps(report, indent=2)+'\n')
        chosen = ['hero', 'rear-quarter', 'bottom-quarter'] if args.quick else list(VIEWS)
        for name in chosen:
            camera, up = VIEWS[name]
            render(model, camera, up).save(folder/'previews'/f'{name}.png')
        print(ship, report['triangles'], 'triangles, all', report['closedComponents'], 'components closed; rendered', len(chosen), 'views', flush=True)
    if not args.ship:
        sheet = Image.new('RGB', (1650, 1420), '#101820')
        draw = ImageDraw.Draw(sheet)
        font_file = '/System/Library/Fonts/Supplemental/Arial.ttf'
        title_font = ImageFont.truetype(font_file, 24)
        label_font = ImageFont.truetype(font_file, 18)
        draw.text((30, 18), 'PIRATE FLEET / EXPORTED LOW-POLYGON MODELS', font=title_font, fill='#e5d6bd')
        for col, (ship, title) in enumerate(zip(SHIPS, TITLES)):
            folder = ROOT.parent/f'{ship}-textured-v1'
            for row, view in enumerate(['hero', 'rear-quarter', 'bottom-quarter']):
                picture = Image.open(folder/'previews'/f'{view}.png').resize((550, 425), Image.Resampling.LANCZOS)
                sheet.paste(picture, (col*550, 100+row*430))
            stats = json.loads((folder/'model-info.json').read_text())
            draw.text((col*550+26, 60), title, font=label_font, fill='#eee3d1')
            draw.text((col*550+26, 84), f"{stats['triangles']:,} triangles · {stats['renderSections']} sections", font=label_font, fill='#bba98b')
        draw.text((30, 1395), 'Software renders of the exported GLBs • Top/front quarter, rear quarter, underside quarter', font=label_font, fill='#92a0b0')
        sheet.save(ROOT/'pirate-fleet-models.png')


if __name__ == '__main__':
    main()
