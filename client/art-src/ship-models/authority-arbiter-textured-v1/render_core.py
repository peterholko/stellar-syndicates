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
        # Respect the real glTF hierarchy: turret meshes are authored around
        # local yaw pivots, while hull/halo assemblies stay at the origin.
        def local_matrix(node):
            if 'matrix' in node:
                return np.asarray(node['matrix'],float).reshape(4,4).T
            x,y,z,w=node.get('rotation',[0,0,0,1])
            rotation=np.array([[1-2*y*y-2*z*z,2*x*y-2*z*w,2*x*z+2*y*w],
                               [2*x*y+2*z*w,1-2*x*x-2*z*z,2*y*z-2*x*w],
                               [2*x*z-2*y*w,2*y*z+2*x*w,1-2*x*x-2*y*y]])
            mat=np.eye(4);mat[:3,:3]=rotation@np.diag(node.get('scale',[1,1,1]))
            mat[:3,3]=node.get('translation',[0,0,0]);return mat
        instances=[]
        self.node_world={}
        def visit(index,parent):
            node=self.doc['nodes'][index];world=parent@local_matrix(node)
            self.node_world[node.get('name',str(index))]=world
            if 'mesh' in node:instances.append((self.doc['meshes'][node['mesh']],world))
            for child in node.get('children',[]):visit(child,world)
        for node in self.doc['scenes'][self.doc.get('scene',0)]['nodes']:visit(node,np.eye(4))
        for mesh,world in instances:
            for primitive in mesh['primitives']:
                a = primitive['attributes']
                position=self.accessor(a['POSITION'])@world[:3,:3].T+world[:3,3]
                normal=self.accessor(a['NORMAL'])@np.linalg.inv(world[:3,:3])
                normal/=np.linalg.norm(normal,axis=1,keepdims=True)
                self.primitives.append({
                    'p': position, 'n': normal,
                    'uv': self.accessor(a['TEXCOORD_0']),
                    'color': self.accessor(a['COLOR_0']) if 'COLOR_0' in a else np.ones_like(position),
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


def bilinear(texture, uv, wrap_s=False, wrap_t=False):
    h, w = texture.shape[:2]
    xy = np.clip(uv, 0, 1)*np.array([w-1, h-1])
    if wrap_s:
        xy[:, 0] = np.mod(uv[:, 0], 1)*w-.5
    if wrap_t:
        xy[:, 1] = np.mod(uv[:, 1], 1)*h-.5
    a = np.floor(xy).astype(int)
    b = a+1
    mix = xy-a
    for axis, size, repeat in [(0, w, wrap_s), (1, h, wrap_t)]:
        a[:, axis] = np.mod(a[:, axis], size) if repeat else np.clip(a[:, axis], 0, size-1)
        b[:, axis] = np.mod(b[:, axis], size) if repeat else np.clip(b[:, axis], 0, size-1)
    x, y = mix[:, 0:1], mix[:, 1:2]
    return ((texture[a[:, 1], a[:, 0]]*(1-x)+texture[a[:, 1], b[:, 0]]*x)*(1-y)+
            (texture[b[:, 1], a[:, 0]]*(1-x)+texture[b[:, 1], b[:, 0]]*x)*y)


def render(model, camera, up=(0, 1, 0), width=1100, height=850, framing_points=None,
           background=None, light_scale=1, return_mask=False, padding=100):
    # Match the GLB's LINEAR_MIPMAP_LINEAR sampler for minified surfaces.
    # This prevents false speckling from sampling the full atlas on tiny faces.
    if not hasattr(model, 'mipmaps'):
        model.mipmaps = []
        for source in model.images:
            levels = [source]
            tex = Image.fromarray(np.round(source*255).astype('uint8'))
            while min(tex.size) > 1:
                tex = tex.resize((max(1, tex.width//2), max(1, tex.height//2)), Image.Resampling.BOX)
                levels.append(np.asarray(tex, dtype=np.float32)/255)
            model.mipmaps.append(levels)
    def filtered(texture_index, uv, tri_uv, screen, wrap):
        source = model.doc['textures'][texture_index]['source']
        levels = model.mipmaps[source]
        pixel_edges = np.stack([screen[1, :2]-screen[0, :2], screen[2, :2]-screen[0, :2]])
        uv_edges = np.stack([tri_uv[1]-tri_uv[0], tri_uv[2]-tri_uv[0]])
        gradient = np.linalg.solve(pixel_edges, uv_edges)*np.array([levels[0].shape[1], levels[0].shape[0]])
        footprint = max(np.linalg.norm(gradient[0]), np.linalg.norm(gradient[1]), 1)
        lod = min(float(np.log2(footprint)), len(levels)-1)
        lo = int(lod); hi = min(lo+1, len(levels)-1); mix = lod-lo
        return bilinear(levels[lo], uv, *wrap)*(1-mix)+bilinear(levels[hi], uv, *wrap)*mix
    def wrapping(texture_index):
        index = model.doc['textures'][texture_index].get('sampler')
        sampler = model.doc.get('samplers', [])[index] if index is not None else {}
        return (sampler.get('wrapS', 10497) == 10497, sampler.get('wrapT', 10497) == 10497)
    direction = unit(camera)
    right = unit(np.cross(up, direction))
    vertical = unit(np.cross(direction, right))
    basis = np.column_stack([right, vertical, direction])
    transformed = (model.positions if framing_points is None else np.asarray(framing_points))@basis
    low, high = transformed.min(axis=0), transformed.max(axis=0)
    middle = (low+high)/2
    scale = min((width-padding)/(high[0]-low[0]), (height-padding)/(high[1]-low[1]))
    frame = np.empty((height, width, 3), dtype=np.float32)
    # A quiet charcoal studio background. No model material is hidden by bloom.
    yy, xx = np.mgrid[:height, :width]
    glow = np.exp(-((xx-width*.50)**2/(width*.62)**2+(yy-height*.45)**2/(height*.65)**2))
    frame[:] = np.array([.032, .043, .058])+glow[:, :, None]*np.array([.024, .030, .036])
    if background is not None:
        frame[:] = np.asarray(background, dtype=np.float32)
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
        color_wrap = wrapping(pbr['baseColorTexture']['index']) if color_tex is not None else (False, False)
        em = primitive['m'].get('emissiveTexture')
        emissive_tex = model.images[model.doc['textures'][em['index']]['source']] if em else None
        emissive_wrap = wrapping(em['index']) if em else (False, False)
        em_factor = np.array(primitive['m'].get('emissiveFactor', [0, 0, 0]))
        for indices in primitive['i']:
            points = primitive['p'][indices]
            face = np.cross(points[1]-points[0], points[2]-points[0])
            if face@direction <= 1e-10 and not primitive['m'].get('doubleSided'):
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
            if primitive['m'].get('doubleSided') and face@direction < 0:
                normals *= -1
            color = np.power(filtered(pbr['baseColorTexture']['index'], uv, primitive['uv'][indices], v, color_wrap), 2.2)*basefactor if color_tex is not None else np.broadcast_to(basefactor, (len(uv), 3)).copy()
            color *= bary@primitive['color'][indices]
            light = .50+.38*np.clip(normals@key, 0, 1)+.18*np.clip(normals@fill, 0, 1)
            color = color*light[:, None]*light_scale if 'KHR_materials_unlit' not in primitive['m'].get('extensions', {}) else color
            if emissive_tex is not None:
                color += np.power(filtered(em['index'], uv, primitive['uv'][indices], v, emissive_wrap), 2.2)*em_factor
            else:
                color += em_factor
            half = unit(key+direction)
            specular = np.power(np.clip(normals@half, 0, 1), 38)*.035
            color += specular[:, None]*light_scale
            rgb = np.power(np.clip(color, 0, 1), 1/2.2)
            frame[miny:maxy+1, minx:maxx+1][mask] = rgb
            area_depth[mask] = d[mask]
    result = Image.fromarray(np.round(np.clip(frame, 0, 1)*255).astype('uint8'))
    return (result,np.isfinite(depth)) if return_mask else result
