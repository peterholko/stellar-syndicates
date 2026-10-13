"""Reference-textured industrial hulls, using the existing NumPy fleet workflow.

Author X/Y is the plan plane, Z is height. Exported assets use Y up, -Z front.
Every physical face has non-collapsed reference UVs, including caps and walls.
"""
from pathlib import Path
from collections import defaultdict
import json
import math
import shutil
import struct
import numpy as np
from PIL import Image
from pipeline_geometry import unit, triangulate


class Reconstruction:
    def __init__(self, root, spec):
        self.root, self.spec = Path(root), spec
        self.parts, self.sockets = [], []
        self.materials = []
        for name in ('Reference titanium and industrial fittings', 'Ventral reference plating'):
            self.materials.append(dict(name=name, pbrMetallicRoughness=dict(
                baseColorFactor=[1, 1, 1, 1], metallicFactor=1, roughnessFactor=1,
                baseColorTexture={'index': 0}, metallicRoughnessTexture={'index': 1}),
                emissiveTexture={'index': 2}, emissiveFactor=[.65, .65, .65]))
        self.textures = [spec['id']+'-'+suffix+'.png' for suffix in
                         ('basecolor', 'metallic-roughness', 'emissive')]

    def point(self, x, y, z=0):
        cx, cy = self.spec['center']
        sx, sy = self.spec['pixelsPerUnit']
        return np.array([(x-cx)/sx, (cy-y)/sy, z], float)

    def prepare_textures(self):
        out = self.root/'textures'
        out.mkdir(exist_ok=True)
        src = self.root.parents[1]/self.spec['reference']
        shutil.copy2(src, out/'reference-sheet.png')
        shutil.copy2(self.root.parents[1]/self.spec['sprite'], self.root/'concept.png')
        rgb = np.asarray(Image.open(src).convert('RGB'), dtype=np.float32)
        atlas = np.full((2048, 2048, 3), (62, 67, 74), np.float32)
        # Native source pixels remain intact. Only empty white margins are
        # extended, exactly as in the Construction Ship texture preparation.
        for box in self.spec['regions'].values():
            x0, y0, x1, y1 = box
            tile = rgb[y0:y1, x0:x1].copy()
            known = tile.min(axis=2) < 237
            for _ in range(36):
                padded = np.pad(tile*known[:, :, None], ((1, 1), (1, 1), (0, 0)))
                valid = np.pad(known.astype(np.float32), 1)
                total, count = np.zeros_like(tile), np.zeros_like(known, dtype=np.float32)
                h, w = known.shape
                for dy, dx in ((0, 1), (1, 0), (1, 2), (2, 1), (0, 0), (0, 2), (2, 0), (2, 2)):
                    total += padded[dy:dy+h, dx:dx+w]
                    count += valid[dy:dy+h, dx:dx+w]
                fill = (~known) & (count > 0)
                tile[fill] = total[fill]/count[fill, None]
                known |= fill
            tile[~known] = (62, 67, 74)
            atlas[y0:y1, x0:x1] = tile
        # Supporting flat material swatches derived from both references by
        # ImageGen cover hidden edges, avoiding enlargement of tiny side crops.
        # Keep the original 2172x724 generation alongside this build script.
        strip=Image.open(Path(__file__).parent/'material-strip.png').convert('RGB')
        strip=strip.resize((2048,round(strip.height*2048/strip.width)),Image.Resampling.LANCZOS)
        atlas[1024:1024+strip.height,:2048]=np.asarray(strip)
        self.spec['patches'].update(silver=[8,1032,674,1698],dark=[692,1032,1355,1698],
                                   brass=[1374,1032,2038,1698],enginewall=[692,1032,1355,1698])
        Image.fromarray(atlas.astype('uint8')).save(out/self.textures[0])
        r, g, b = atlas.transpose(2, 0, 1)
        cyan = np.clip((g-r-28)/95, 0, 1)*np.clip((b-r-32)/95, 0, 1)
        Image.fromarray(np.clip(atlas*cyan[:, :, None], 0, 255).astype('uint8')).save(out/self.textures[2])
        value = atlas.mean(axis=2)/255
        rough, metal = np.where(value > .52, .58, .48), np.where(value > .52, .32, .40)
        brass = (r > b*1.3) & (r > 95) & (g > b*1.12)
        rough[brass], metal[brass] = .40, .55
        rough[cyan > .3], metal[cyan > .3] = .26, .08
        mr = np.stack([np.ones_like(r), rough, metal], axis=2)
        Image.fromarray((mr*255).astype('uint8')).save(out/self.textures[1])

    def plan_uv(self, points, bottom=False):
        sx, sy = self.spec['pixelsPerUnit']
        cx, cy = self.spec['center']
        pixels = np.column_stack([cx+points[:, 0]*sx, cy-points[:, 1]*sy])
        if bottom:
            top = self.spec['regions']['top']
            bot = self.spec['regions']['bottom']
            pixels[:, 0] = bot[2]-(pixels[:, 0]-top[0])*(bot[2]-bot[0])/(top[2]-top[0])
            pixels[:, 1] = bot[1]+(pixels[:, 1]-top[1])*(bot[3]-bot[1])/(top[3]-top[1])
        return pixels/2048

    def face_uv(self, points, normal, patch):
        """Planar patch mapping at uniform scale: no stretched or collapsed caps."""
        edges = np.roll(points, -1, axis=0)-points
        horizontal = unit(edges[np.argmax(np.linalg.norm(edges, axis=1))])
        vertical = unit(np.cross(normal, horizontal))
        xy = points@np.column_stack([horizontal, vertical])
        center = (xy.min(axis=0)+xy.max(axis=0))/2
        rect = np.array(self.spec['patches'][patch], float).reshape(2, 2)
        available = rect[1]-rect[0]-2
        scale = np.min(available/np.maximum(np.ptp(xy, axis=0), 1e-12))
        return ((rect[0]+rect[1])/2+(xy-center)*scale)/2048

    def mesh(self, name, vertices, faces, style='auto', mapping=None, smooth=False, assembly='hull'):
        v = np.asarray(vertices, float)
        faces = [list(f) for f in faces]
        volume = sum(np.dot(v[f[0]], np.cross(v[f[j]], v[f[j+1]]))
                     for f in faces for j in range(1, len(f)-1))/6
        if volume < 0:
            faces = [f[::-1] for f in faces]
        normals = [unit(np.sum(np.cross(v[f], np.roll(v[f], -1, axis=0)), axis=0)) for f in faces]
        adjacent = [[] for _ in v]
        for fi, f in enumerate(faces):
            for vi in f:
                adjacent[vi].append(fi)
        groups = {}
        for fi, face in enumerate(faces):
            normal = normals[fi]
            p = v[face]
            tris = triangulate(p)
            mat = int(style == 'auto' and normal[2] < -.55)
            if mapping:
                uv = mapping(p, normal, fi)
            elif style == 'auto' and abs(normal[2]) > .55:
                uv = self.plan_uv(p, normal[2] < 0)
            else:
                uv = self.face_uv(p, normal, 'silver' if style == 'auto' else style)
            t = uv[np.asarray(tris)]
            a, b = t[:, 1]-t[:, 0], t[:, 2]-t[:, 0]
            if np.abs(a[:, 0]*b[:, 1]-a[:, 1]*b[:, 0]).min() < 1e-12:
                uv = self.face_uv(p, normal, 'silver' if style == 'auto' else style)
            g = groups.setdefault(mat, dict(position=[], normal=[], uv=[], indices=[]))
            start = len(g['position'])
            for j, vi in enumerate(face):
                vn = normal
                if smooth:
                    vn = unit(sum((normals[i] for i in adjacent[vi] if normals[i]@normal > .75), np.zeros(3)))
                g['position'].append(v[vi])
                g['normal'].append(vn)
                g['uv'].append(uv[j])
            for triangle in tris:
                g['indices'].extend(start+i for i in triangle)
        self.parts.append(dict(name=name, assembly=assembly, groups=groups, closed=True))

    def slab(self, name, outline, low, high, bevel=.06, style='auto', assembly='hull'):
        """Closed chamfered extrusion, preserving concave footprint openings."""
        xy = np.array([self.point(x, y)[:2] for x, y in outline])
        # Inset toward centroid only for convex armor slabs. Concave cradles
        # are assembled from separate closed rails, never filled by this cap.
        center = xy.mean(axis=0)
        inset = center+(xy-center)*max(.75, 1-bevel/max(np.ptp(xy, axis=0).min()/2, .01))
        bz = min(bevel, (high-low)/3)
        vertices = [np.r_[p, z] for ring, z in ((inset, low), (xy, low+bz),
                    (xy, high-bz), (inset, high)) for p in ring]
        n = len(xy)
        faces = [tuple(reversed(range(n))), tuple(range(3*n, 4*n))]
        faces += [(k*n+i, k*n+(i+1)%n, (k+1)*n+(i+1)%n, (k+1)*n+i)
                  for k in range(3) for i in range(n)]
        self.mesh(name, vertices, faces, style, assembly=assembly)

    def box(self, name, bounds, low, high, style='auto', bevel=.06, corner=4):
        x0, y0, x1, y1 = bounds
        c = min(corner, (x1-x0)/3, (y1-y0)/3)
        self.slab(name, [(x0+c,y0),(x1-c,y0),(x1,y0+c),(x1,y1-c),
                        (x1-c,y1),(x0+c,y1),(x0,y1-c),(x0,y0+c)], low, high, bevel, style)

    def beam(self, name, a, b, width, depth, style='dark'):
        a, b = np.array(a), np.array(b)
        direction = unit(b-a)
        helper = [0, 0, 1] if abs(direction[2]) < .95 else [1, 0, 0]
        right = unit(np.cross(direction, helper))*width/2
        up = unit(np.cross(direction, right))*depth/2
        vertices = [p+s*right+t*up for p in (a,b) for s,t in ((-1,-1),(1,-1),(1,1),(-1,1))]
        self.mesh(name, vertices, [(0,3,2,1),(4,5,6,7),(0,1,5,4),
                                   (1,2,6,5),(2,3,7,6),(3,0,4,7)], style)

    def lathe(self, name, center, axis, rows, segments=24, style='auto', wall='silver', cap=None, ring=False):
        """Closed lathe, continuously unwrapped around arbitrary axis.

        rows are (axial distance, radius). A ring connects last row to first;
        other profiles have real planar end caps, never zero-radius fans.
        """
        center, axis = np.array(center, float), unit(axis)
        helper = [0, 1, 0] if abs(axis[2]) > .9 else [0, 0, 1]
        right = unit(np.cross(axis, helper))
        up = unit(np.cross(axis, right))
        vertices = [center+axis*h+r*(right*math.cos(i*math.tau/segments)+up*math.sin(i*math.tau/segments))
                    for h,r in rows for i in range(segments)]
        count = len(rows) if ring else len(rows)-1
        faces = [(k*segments+i,k*segments+(i+1)%segments,((k+1)%len(rows))*segments+(i+1)%segments,
                  ((k+1)%len(rows))*segments+i) for k in range(count) for i in range(segments)]
        if not ring:
            faces += [tuple(reversed(range(segments))), tuple(range(len(vertices)-segments, len(vertices)))]
        hmin, hmax = min(r[0] for r in rows), max(r[0] for r in rows)
        rmax = max(r[1] for r in rows)
        rect = np.array(self.spec['patches'][wall],float).reshape(2,2)
        def mapping(p,n,fi):
            if abs(n[2]) > .65 and style == 'auto':
                return self.plan_uv(p, n[2] < 0)
            if abs(n@axis) > .8:
                if cap and fi == count*segments+1:
                    cx, cy, radius = cap
                    local = p-center
                    return np.column_stack([cx+(local@right)/rmax*radius,
                                            cy-(local@up)/rmax*radius])/2048
                return self.face_uv(p,n,'silver' if style=='auto' else style)
            local = p-center
            angles = np.unwrap(np.arctan2(local@up,local@right))
            if angles.min() < -1e-7: angles += math.tau
            u = angles/math.tau
            if wall=='silowall':
                # Four native elevation panels around each tank preserve
                # texel density instead of stretching one panel 360 degrees.
                u=u*4
                u-=math.floor(float(u.min())+1e-7)
            h = 1-(local@axis-hmin)/max(hmax-hmin, 1e-9)
            return (rect[0]+np.column_stack([u,h])*(rect[1]-rect[0]))/2048
        self.mesh(name,vertices,faces,style,mapping=mapping,smooth=True)

    def cylinder(self,name,a,b,r,style='silver',segments=12):
        a,b = np.array(a),np.array(b)
        self.lathe(name,a,b-a,[(0,r),(np.linalg.norm(b-a),r)],segments,style,style)

    def socket(self,name,position,direction,role,**extra):
        axis = lambda v: [float(v[0]), float(v[2]), -float(v[1])]
        self.sockets.append(dict(name=name,translation=axis(position),
                                 extras=dict(role=role,direction=axis(direction),**extra)))

    def export(self):
        grouped = {}
        for part in self.parts:
            for mat,g in part['groups'].items():
                out = grouped.setdefault((part['assembly'],mat),dict(position=[],normal=[],uv=[],indices=[],components=[]))
                out['components'].append(dict(name=part['name'],indexStart=len(out['indices']),indexCount=len(g['indices']),closed=True))
                offset = len(out['position'])
                for key in ('position','normal','uv'): out[key].extend(g[key])
                out['indices'].extend(offset+i for i in g['indices'])
        doc = dict(asset={'version':'2.0','generator':'Stellar Syndicates Python/NumPy reconstruction; no Blender'},
                   scene=0,scenes=[{'nodes':[0]}],nodes=[{'name':self.spec['name'],'children':[],
                   'extras':{'axes':'Y up, -Z forward','sourceReference':self.spec['reference']}}],
                   materials=self.materials,meshes=[],buffers=[{}],bufferViews=[],accessors=[],images=[],textures=[],
                   samplers=[{'magFilter':9729,'minFilter':9987,'wrapS':33071,'wrapT':33071}])
        binary = bytearray()
        def view(data,target=None):
            binary.extend(b'\0'*(-len(binary)%4))
            v = dict(buffer=0,byteOffset=len(binary),byteLength=len(data))
            if target: v['target']=target
            binary.extend(data);doc['bufferViews'].append(v)
            return len(doc['bufferViews'])-1
        def accessor(data,component,kind,target,bounds=False):
            a=np.asarray(data,dtype={5126:'<f4',5123:'<u2',5125:'<u4'}[component])
            acc=dict(bufferView=view(a.tobytes(),target),componentType=component,count=len(a),type=kind)
            if bounds: acc.update(min=a.min(axis=0).tolist(),max=a.max(axis=0).tolist())
            doc['accessors'].append(acc);return len(doc['accessors'])-1
        for texture in self.textures:
            doc['images'].append(dict(name=texture,mimeType='image/png',bufferView=view((self.root/'textures'/texture).read_bytes())))
            doc['textures'].append({'source':len(doc['images'])-1,'sampler':0})
        positions=[];triangles=0
        for (assembly,mat),g in grouped.items():
            packed=np.column_stack([g['position'],g['normal'],g['uv']]).astype('<f4')
            unique,inverse=np.unique(packed,axis=0,return_inverse=True)
            p=unique[:,[0,2,1]]*np.array([1,1,-1]);n=unique[:,[3,5,4]]*np.array([1,1,-1])
            indices=inverse[g['indices']]
            attrs={key:accessor(data,5126,kind,34962,key=='POSITION') for key,data,kind in
                   [('POSITION',p,'VEC3'),('NORMAL',n,'VEC3'),('TEXCOORD_0',unique[:,6:8],'VEC2')]}
            primitive=dict(attributes=attrs,indices=accessor(indices,5123 if len(p)<65536 else 5125,'SCALAR',34963),material=mat)
            name=assembly+' / '+self.materials[mat]['name']
            doc['meshes'].append(dict(name=name,primitives=[primitive],extras={'sourceComponents':g['components']}))
            doc['nodes'].append(dict(name=name,mesh=len(doc['meshes'])-1));doc['nodes'][0]['children'].append(len(doc['nodes'])-1)
            positions.extend(p);triangles+=len(indices)//3
        for socket in self.sockets:
            doc['nodes'].append(socket);doc['nodes'][0]['children'].append(len(doc['nodes'])-1)
        doc['buffers'][0]['byteLength']=len(binary)
        encoded=json.dumps(doc,separators=(',',':')).encode();encoded+=b' '*(-len(encoded)%4)
        binary.extend(b'\0'*(-len(binary)%4))
        result=struct.pack('<4sII',b'glTF',2,28+len(encoded)+len(binary))
        result+=struct.pack('<I4s',len(encoded),b'JSON')+encoded+struct.pack('<I4s',len(binary),b'BIN\0')+binary
        stem=self.spec['id'];(self.root/(stem+'.glb')).write_bytes(result)
        info=dict(name=self.spec['name'],triangles=triangles,renderSections=len(grouped),sourceComponents=len(self.parts),
                  materials=len(self.materials),embeddedTextures=len(self.textures),textureResolution=[2048,2048],
                  dimensionsXYZ=np.ptp(np.array(positions),axis=0).round(4).tolist(),axes='Y up, -Z forward',
                  units='Relative model units; gameplay scale is assigned at integration',sockets=self.sockets,
                  geometryCounts=self.spec['counts'],triangleBudget=self.spec['triangleBudget'],
                  generator='Python/NumPy, existing fleet reconstruction workflow; no Blender',
                  textures='Native five-view pixels; dedicated bottom projection; continuous cylinder walls; reference-matched ImageGen material swatches on hidden edges and fittings',
                  optimization='Static parts merged by material; exact vertex deduplication; 16-bit indices',
                  note='Concept reconstruction with standardized orthographic proportions. No gameplay or effects wiring.')
        (self.root/'model-info.json').write_text(json.dumps(info,indent=2)+'\n')
        (self.root/'model-spec.json').write_text(json.dumps(self.spec,indent=2)+'\n')
        lines=[f'# {self.spec["name"]}; Y up, -Z forward',f'mtllib {stem}.mtl'];offset=1
        for part in self.parts:
            lines.append('o '+part['name'])
            for mat,g in part['groups'].items():
                lines.append(f'usemtl material_{mat}')
                lines.extend(f'v {x:.6f} {z:.6f} {-y:.6f}' for x,y,z in g['position'])
                lines.extend(f'vt {u:.7f} {1-v:.7f}' for u,v in g['uv'])
                lines.extend(f'vn {x:.7f} {z:.7f} {-y:.7f}' for x,y,z in g['normal'])
                for i in range(0,len(g['indices']),3):
                    lines.append('f '+' '.join(f'{j+offset}/{j+offset}/{j+offset}' for j in g['indices'][i:i+3]))
                offset+=len(g['position'])
        (self.root/(stem+'.obj')).write_text('\n'.join(lines)+'\n')
        mtl=[]
        for i in range(len(self.materials)):
            mtl.extend([f'newmtl material_{i}','Kd 1 1 1','Ks .3 .3 .3','Ns 48','Ke .65 .65 .65',
                        'map_Kd textures/'+self.textures[0],'map_Ke textures/'+self.textures[2],''])
        (self.root/(stem+'.mtl')).write_text('\n'.join(mtl))
        assert self.spec['triangleBudget'][0]<=triangles<=self.spec['triangleBudget'][1], triangles
        print(json.dumps({k:info[k] for k in ('name','triangles','renderSections','sourceComponents','dimensionsXYZ')},indent=2))
