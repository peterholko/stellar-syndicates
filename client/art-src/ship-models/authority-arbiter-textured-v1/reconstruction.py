"""Authority Arbiter reconstruction using the existing NumPy fleet workflow.

Author X/Y is the plan plane, Z is height. Exported assets use Y up, -Z front.
Every physical face has non-collapsed reference UVs, including caps and walls.
"""
from pathlib import Path
from collections import defaultdict
import json
import hashlib
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
        self.current_assembly = 'hull'
        self.assemblies = {'hull': {'pivot': [0,0,0], 'role': 'static-armor'}}
        self.materials = []
        for name in ('Authority ivory, navy and antique gold', 'Ventral hangar and keel plating'):
            self.materials.append(dict(name=name, pbrMetallicRoughness=dict(
                baseColorFactor=[1, 1, 1, 1], metallicFactor=1, roughnessFactor=1,
                baseColorTexture={'index': 0}, metallicRoughnessTexture={'index': 1}),
                emissiveTexture={'index': 2}, emissiveFactor=[.65, .65, .65]))
        self.textures = [spec['id']+'-basecolor.jpg',spec['id']+'-metallic-roughness.png',spec['id']+'-emissive.png']
        self.texture_resolution=1024
        self.mapping_seed=0

    def point(self, x, y, z=0):
        cx, cy = self.spec['center']
        sx, sy = self.spec['pixelsPerUnit']
        return np.array([(x-cx)/sx, (cy-y)/sy, z], float)

    def prepare_textures(self):
        """Pack clean material swatches and isolated lamps; never project ship art.

        UV coordinates use a virtual 2048 grid, packed to 1024 at export. JPEG
        albedo and PNG parameter/emission maps keep both GLBs below 3 MB.
        """
        out=self.root/'textures';out.mkdir(exist_ok=True)
        src=self.root.parents[1]/self.spec['reference']
        shutil.copy2(src,out/'reference-sheet.png')
        shutil.copy2(self.root.parents[1]/self.spec['sprite'],self.root/'concept.png')
        sheet=Image.open(src).convert('RGB')
        atlas=Image.open(self.root/'materials/material-atlas-artpass-v2.png').convert('RGB').resize((2048,2048),Image.Resampling.LANCZOS)
        emission=Image.new('RGB',(2048,2048))
        lamps=[((1294,156,1336,169),(1776,1760,1872,1792),'amber'),
               ((1002,207,1022,216),(1900,1760,1996,1792),'cyan'),
               ((1163,482,1231,550),(1848,1888,1960,2000),'cyan')]
        for source_box,destination,kind in lamps:
            x0,y0,x1,y1=destination
            tile=sheet.crop(source_box).resize((x1-x0,y1-y0),Image.Resampling.LANCZOS)
            # Identical edge pixels pad the small light island for mipmapping.
            a=np.asarray(tile)
            atlas.paste(Image.fromarray(np.pad(a,((12,12),(12,12),(0,0)),mode='edge')),(x0-12,y0-12))
            rgb=np.asarray(tile,dtype=float);r,g,b=rgb.transpose(2,0,1)
            mask=np.clip((r-b-35)/100,0,1) if kind=='amber' else np.clip((g-r-20)/95,0,1)*np.clip((b-r-20)/95,0,1)
            emission.paste(Image.fromarray((rgb*mask[:,:,None]).astype('uint8')),(x0,y0))
        self.spec['patches'].update(silver=[18,18,1006,1006],brass=[1042,18,2030,1006],
            dark=[18,1042,1006,2030],enginewall=[18,1042,1006,2030],
            service=[1042,1042,1680,2030],vent=[1042,1042,1680,2030],
            muzzle=[1140,1330,1580,1570],grille=[1580,1120,2018,1238],
            amber=[1780,1764,1868,1788],cyan=[1904,1764,1992,1788])
        # Parameter maps are data: clean masks for ceramic, brass, machinery and lamps.
        mr=np.empty((2048,2048,3),np.uint8);mr[:]=[255,145,65]
        mr[:1024,1024:]=[255,112,190]
        mr[1024:,:1024]=[255,142,145]
        mr[1024:,1024:]=[255,127,160]
        for _,(x0,y0,x1,y1),_ in lamps:mr[y0:y1,x0:x1]=[255,70,20]
        size=(self.texture_resolution,self.texture_resolution)
        atlas.resize(size,Image.Resampling.LANCZOS).save(out/self.textures[0],quality=94,subsampling=0,optimize=True)
        emission.resize(size,Image.Resampling.LANCZOS).save(out/self.textures[2],optimize=True)
        Image.fromarray(mr).resize(size,Image.Resampling.NEAREST).save(out/self.textures[1],optimize=True)
        self.spec['texturePipeline']='Clean imagegen material swatches, physical face UVs, dedicated light islands; 1024px JPEG albedo and PNG PBR maps'

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
        # A small bevel must not receive an entire wall's worth of panel seams.
        # Keep a consistent texture density, reducing it only for very large
        # single faces that exceed this atlas patch.
        density={'silver':230,'dark':160,'enginewall':160,'brass':130,'service':190,'vent':190,'muzzle':100,'grille':10000,'amber':10000,'cyan':10000}.get(patch,90)
        scale = min(density,float(np.min(available/np.maximum(np.ptp(xy, axis=0), 1e-12))))
        spare=np.maximum(available-np.ptp(xy,axis=0)*scale,0)
        offset=np.array([((self.mapping_seed%97)/96-.5),(((self.mapping_seed//97)%89)/88-.5)])*spare*.76
        if patch in ('amber','cyan','grille'):offset*=0
        return ((rect[0]+rect[1])/2+offset+(xy-center)*scale)/2048

    def mesh(self, name, vertices, faces, style='auto', mapping=None, smooth=False, assembly=None):
        assembly=assembly or self.current_assembly
        self.mapping_seed=int.from_bytes(hashlib.sha256(name.encode()).digest()[:4],'little')
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
            moving=self.assemblies.get(assembly,{}).get('role') in ('turret-yaw','secondary-turret-yaw','point-defense-yaw')
            mat = int(style == 'auto' and normal[2] < -.55 and not moving)
            if mapping:
                uv = mapping(p, normal, fi)
            elif style=='grille':
                uv = self.face_uv(p, normal, 'grille' if abs(normal[2])>.6 else 'dark')
            elif style == 'auto':
                uv = self.face_uv(p, normal, 'service' if normal[2]<-.55 else 'silver')
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

    def slab(self, name, outline, low, high, bevel=.06, style='auto', assembly=None):
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
            circum=angles*rmax
            height=local@axis-(hmin+hmax)/2
            density=min(160,(rect[1,0]-rect[0,0]-8)/(math.tau*rmax),
                        (rect[1,1]-rect[0,1]-8)/max(hmax-hmin,1e-8))
            return ((rect[0]+rect[1])/2+np.column_stack([circum-math.pi*rmax,-height])*density)/2048

        self.mesh(name,vertices,faces,style,mapping=mapping,smooth=True)

    def cylinder(self,name,a,b,r,style='silver',segments=12):
        a,b = np.array(a),np.array(b)
        self.lathe(name,a,b-a,[(0,r),(np.linalg.norm(b-a),r)],segments,style,style)

    def socket(self,name,position,direction,role,**extra):
        axis = lambda v: [float(v[0]), float(v[2]), -float(v[1])]
        self.sockets.append(dict(name=name,translation=axis(position),
                                 extras=dict(role=role,direction=axis(direction),**extra)))

    def assembly(self,name,pivot=(0,0,0),role='static-armor'):
        self.current_assembly=name
        self.assemblies[name]={'pivot':list(pivot),'role':role}

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
            doc['images'].append(dict(name=texture,mimeType='image/jpeg' if texture.endswith('.jpg') else 'image/png',bufferView=view((self.root/'textures'/texture).read_bytes())))
            doc['textures'].append({'source':len(doc['images'])-1,'sampler':0})
        positions=[];triangles=0;assembly_nodes={}
        for (assembly,mat),g in grouped.items():
            settings=self.assemblies.get(assembly,{'pivot':[0,0,0],'role':'static-armor'})
            author_pivot=np.asarray(settings['pivot'])
            pivot=author_pivot[[0,2,1]]*np.array([1,1,-1])
            if assembly not in assembly_nodes:
                doc['nodes'].append(dict(name=assembly,translation=pivot.tolist(),children=[],extras={'role':settings['role']}))
                assembly_nodes[assembly]=len(doc['nodes'])-1
                doc['nodes'][0]['children'].append(len(doc['nodes'])-1)
            packed=np.column_stack([g['position'],g['normal'],g['uv']]).astype('<f4')
            unique,inverse=np.unique(packed,axis=0,return_inverse=True)
            p=unique[:,[0,2,1]]*np.array([1,1,-1]);n=unique[:,[3,5,4]]*np.array([1,1,-1])
            positions.extend(p.copy())
            p-=pivot
            indices=inverse[g['indices']]
            attrs={key:accessor(data,5126,kind,34962,key=='POSITION') for key,data,kind in
                   [('POSITION',p,'VEC3'),('NORMAL',n,'VEC3'),('TEXCOORD_0',unique[:,6:8],'VEC2')]}
            primitive=dict(attributes=attrs,indices=accessor(indices,5123 if len(p)<65536 else 5125,'SCALAR',34963),material=mat)
            name=assembly+' / '+self.materials[mat]['name']
            doc['meshes'].append(dict(name=name,primitives=[primitive],extras={'sourceComponents':g['components']}))
            doc['nodes'].append(dict(name=name,mesh=len(doc['meshes'])-1));doc['nodes'][assembly_nodes[assembly]]['children'].append(len(doc['nodes'])-1)
            triangles+=len(indices)//3
        for socket in self.sockets:
            node=dict(socket)
            parent=assembly_nodes.get(socket['extras'].get('assembly'),0)
            if parent:
                node['translation']=(np.array(socket['translation'])-np.array(doc['nodes'][parent]['translation'])).tolist()
            doc['nodes'].append(node);doc['nodes'][parent]['children'].append(len(doc['nodes'])-1)
        doc['buffers'][0]['byteLength']=len(binary)
        encoded=json.dumps(doc,separators=(',',':')).encode();encoded+=b' '*(-len(encoded)%4)
        binary.extend(b'\0'*(-len(binary)%4))
        result=struct.pack('<4sII',b'glTF',2,28+len(encoded)+len(binary))
        result+=struct.pack('<I4s',len(encoded),b'JSON')+encoded+struct.pack('<I4s',len(binary),b'BIN\0')+binary
        stem=self.spec['id'];(self.root/(stem+'.glb')).write_bytes(result)
        info=dict(name=self.spec['name'],triangles=triangles,renderSections=len(grouped),sourceComponents=len(self.parts),
                  materials=len(self.materials),embeddedTextures=len(self.textures),textureResolution=[self.texture_resolution,self.texture_resolution],
                  dimensionsXYZ=np.ptp(np.array(positions),axis=0).round(4).tolist(),axes='Y up, -Z forward',
                  units='Relative model units; gameplay scale is assigned at integration',sockets=self.sockets,
                  geometryCounts=self.spec['counts'],triangleBudget=self.spec['triangleBudget'],
                  generator='Python/NumPy, existing fleet reconstruction workflow; no Blender',
                  textures='Clean material swatches with face-local physical-scale UVs, continuously wrapped gold halo and engines, dedicated ventral service panels and isolated emissive lamps; 1024px PBR maps',
                  optimization='Static hull merged by material; halo separate; 22 independently pivoted gun mounts; exact vertex deduplication; 16-bit indices',
                  assemblies={k:dict(role=v['role'],pivotYUp=(np.array(v['pivot'])[[0,2,1]]*np.array([1,1,-1])).tolist()) for k,v in self.assemblies.items()},
                  artDirection='Imperial Authority: stepped command tower, three sculpted staggered spines with separate gold-crested ceramic fairings over recessed navy cores, ivory buttresses, amber bridge; a recessed capital mass driver between pointed prow blades, broad gold spine crowns and a continuous gold halo',
                  note='Concept reconstruction with the user-requested imperial architecture and enlarged arsenal. The top-view art pass restores the pointed bow, broad gold emblem spines and bold gold halo. All 45 muzzle sockets use the battle-compatible socket.weapon namespace. No gameplay or effects wiring.',
                  socketCoordinates='model-info.json records rest-pose model coordinates; GLB weapon sockets are children of their gun assemblies and use local coordinates.')
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
