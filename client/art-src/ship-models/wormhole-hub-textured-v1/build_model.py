"""Build a real, textured wormhole hub from the approved reference artwork.

Y is up; the ring lies in XY and its front faces +Z. Geometry is authored in
relative units. No inference service or Blender is required. Run with Python,
NumPy and Pillow. All exports and source inputs stay beside this script.
"""
from pathlib import Path
from collections import defaultdict
import json
import math
import shutil
import struct
import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent
PORTAL_PROFILE = json.loads((ROOT/'portal-profile.json').read_text())
ART = ROOT.parent.parent
TAU = math.tau
PARTS = []
MATERIALS = []
TEXTURES = [
    'reference-front.png', 'titanium-panels.png', 'titanium-metallic-roughness.png',
    'brass-panels.png', 'brass-metallic-roughness.png',
    'power-cells.png', 'power-metallic-roughness.png', 'power-emissive.png',
]


def unit(v):
    v = np.asarray(v, dtype=float)
    return v / max(np.linalg.norm(v), 1e-12)


def material(name, color=(1, 1, 1), metal=.6, rough=.48, texture=None,
             mr=None, emission=None, em_texture=None, unlit=False):
    pbr = dict(baseColorFactor=[*color, 1], metallicFactor=metal, roughnessFactor=rough)
    m = dict(name=name, pbrMetallicRoughness=pbr)
    if texture is not None:
        pbr['baseColorTexture'] = {'index': texture}
    if mr is not None:
        pbr['metallicRoughnessTexture'] = {'index': mr}
    if emission is not None:
        m['emissiveFactor'] = list(emission)
    if em_texture is not None:
        m['emissiveTexture'] = {'index': em_texture}
    if unlit:
        m['extensions'] = {'KHR_materials_unlit': {}}
        m['doubleSided'] = True
    MATERIALS.append(m)
    return len(MATERIALS)-1


REFERENCE = material('Reference-projected steel armor', metal=.46, rough=.56, texture=0)
DARK = material('Graphite structural plating', (.075, .095, .125), .68, 1, 1, 2)
STEEL = material('Titanium panel armor', (.34, .39, .46), .8, 1, 1, 2)
BRASS = material('Warm machined brass', (.65, .53, .32), 1, 1, 3, 4)
BLACK = material('Recessed radiator graphite', (.022, .03, .042), .45, .69, 1, 2)
BLUE = material('Cyan reactor cells', (.38, .73, 1), 1, 1, 5, 6, (.38, .7, 1), 7)
WHITE = material('Electric blue containment light', (.08, .35, .7), .05, .2,
                 emission=(.15, .65, 1))
AMBER = material('Amber navigation lights', (.16, .055, .008), .05, .25,
                 emission=(1, .38, .045))
PORTAL = material('Wormhole membrane / reference galaxy', metal=0, rough=1, texture=0, unlit=True)


def prepare_inputs():
    textures = ROOT/'textures'
    textures.mkdir(parents=True, exist_ok=True)
    front = Path('/Users/peter/.codex/generated_images/01a16175-9fd0-7830-a903-376d2daadb90/exec-1987174b-63fc-4cb1-ad12-bac907cc1a13.png')
    sheet = ART/'wormhole-hub-orthographic-v1/wormhole-hub-orthographic.png'
    if not (textures/TEXTURES[0]).exists():
        shutil.copy2(front, textures/TEXTURES[0])
    if not (ROOT/'reference-sheet.png').exists():
        shutil.copy2(sheet, ROOT/'reference-sheet.png')
    shared = ROOT.parent/'authority-nexus-textured-v1/textures'
    sources = ['authority-nexus-ivory-panels.png', 'authority-nexus-ivory-metallic-roughness.png',
               'authority-nexus-brass-ring.png', 'authority-nexus-brass-ring-metallic-roughness.png',
               'authority-nexus-power-cells.png', 'authority-nexus-power-cells-metallic-roughness.png',
               'authority-nexus-power-cells-emissive.png']
    for source, dest in zip(sources, TEXTURES[1:]):
        if not (textures/dest).exists():
            shutil.copy2(shared/source, textures/dest)


def reference_uv(points, back=False):
    # Register the original, undistorted front, not the stretched sheet views.
    # The source image is copied unchanged and sampled directly by these UVs.
    x = -points[:, 0] if back else points[:, 0]
    return np.column_stack([(626+x*71.0)/1254, (602-points[:, 1]*71.0)/1254])


def face_uv(points, normal, material_index):
    if material_index == REFERENCE:
        return reference_uv(points, normal[2] < 0)
    if material_index == PORTAL:
        # Fit the galaxy and electric rim inside the independent circular mesh.
        return np.column_stack([(626+points[:, 0]*70.5)/1254,
                                (603-points[:, 1]*70.5)/1254])
    edges = np.roll(points, -1, axis=0)-points
    right = unit(edges[np.argmax(np.linalg.norm(edges, axis=1))])
    up = unit(np.cross(normal, right))
    local = points@np.column_stack([right, up])
    span = np.maximum(np.ptp(local, axis=0), 1e-7)
    center = (local.min(axis=0)+local.max(axis=0))/2
    limits = np.array([.88, .88])
    if material_index == BRASS:
        limits = np.array([.88, .43])
    scale = np.min(limits/span)
    return np.array([.5, .5])+(local-center)*scale


def mesh(name, vertices, faces, mat=DARK, assembly='station-hardware',
         projected=False, closed=True):
    vertices = np.asarray(vertices, float)
    faces = [list(f) for f in faces]
    volume = sum(np.dot(vertices[f[0]], np.cross(vertices[f[j]], vertices[f[j+1]]))
                 for f in faces for j in range(1, len(f)-1))/6
    if closed and volume < 0:
        faces = [f[::-1] for f in faces]
    groups = {}
    for face in faces:
        points = vertices[face]
        normal = unit(np.sum(np.cross(points, np.roll(points, -1, axis=0)), axis=0))
        chosen = REFERENCE if projected and abs(normal[2]) > .95 else mat
        uv = face_uv(points, normal, chosen)
        uv = np.clip(uv, .0001, .9999)
        g = groups.setdefault(chosen, dict(position=[], normal=[], uv=[], indices=[]))
        offset = len(g['position'])
        g['position'].extend(points)
        g['normal'].extend([normal]*len(points))
        g['uv'].extend(uv)
        for j in range(1, len(face)-1):
            g['indices'].extend([offset, offset+j, offset+j+1])
    PARTS.append(dict(name=name, groups=groups, assembly=assembly, closed=closed))


def annulus(name, profile, mat, segments=128, start=0, sweep=TAU,
            assembly='station-hardware', projected=False):
    full = abs(sweep-TAU) < 1e-7
    count = segments if full else segments+1
    vertices = [(r*math.cos(start+sweep*i/segments), r*math.sin(start+sweep*i/segments), z)
                for r, z in profile for i in range(count)]
    faces = []
    for row in range(len(profile)):
        other = (row+1) % len(profile)
        for i in range(segments):
            nxt = (i+1) % count
            faces.append([row*count+i, row*count+nxt, other*count+nxt, other*count+i])
    if not full:
        faces.extend([[k*count for k in range(len(profile))],
                      [k*count+segments for k in reversed(range(len(profile)))]])
    mesh(name, vertices, faces, mat, assembly, projected)


def ring(name, inner, outer, low, high, mat, **kwargs):
    annulus(name, [(inner, low), (outer, low), (outer, high), (inner, high)], mat, **kwargs)


def box(name, center, dimensions, mat=DARK, angle=0, projected=False, assembly='station-hardware'):
    center, dims = np.asarray(center), np.asarray(dimensions)/2
    c, s = math.cos(angle), math.sin(angle)
    rotation = np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]])
    points = [center+rotation@(np.array(p)*dims) for p in
              [(-1,-1,-1),(1,-1,-1),(1,1,-1),(-1,1,-1),
               (-1,-1,1),(1,-1,1),(1,1,1),(-1,1,1)]]
    mesh(name, points, [(3,2,1,0),(4,5,6,7),(0,1,5,4),(1,2,6,5),(2,3,7,6),(3,0,4,7)],
         mat, assembly, projected)


def beam(name, a, b, radius, mat=BRASS, segments=8, end_radius=None):
    a, b = np.asarray(a, float), np.asarray(b, float)
    axis = unit(b-a)
    helper = [0, 0, 1] if abs(axis[2]) < .9 else [1, 0, 0]
    right = unit(np.cross(axis, helper))
    up = unit(np.cross(axis, right))
    end_radius = radius if end_radius is None else end_radius
    points = [p+r*(math.cos(i*TAU/segments)*right+math.sin(i*TAU/segments)*up)
              for p, r in [(a, radius), (b, end_radius)] for i in range(segments)]
    faces = [list(reversed(range(segments))), list(range(segments, segments*2))]
    faces.extend([(i,(i+1)%segments,(i+1)%segments+segments,i+segments) for i in range(segments)])
    mesh(name, points, faces, mat)


def tapered_spine(name, rows, x=0, y_sign=1, z=0, mat=DARK, projected=True, horizontal=False):
    # An eight-sided, bevel-edged cross section, with physically closed caps.
    points = []
    for y, width, depth in rows:
        w, d, bevel = width/2, depth/2, min(width, depth)*.15
        cross = [(-w+bevel,-d),(w-bevel,-d),(w,-d+bevel),(w,d-bevel),
                 (w-bevel,d),(-w+bevel,d),(-w,d-bevel),(-w,-d+bevel)]
        for xx, zz in cross:
            p = [x+xx, y*y_sign, z+zz]
            if horizontal:
                p = [p[1], p[0], p[2]]
            points.append(p)
    faces = [list(reversed(range(8)))]
    for row in range(len(rows)-1):
        faces.extend([(row*8+i,row*8+(i+1)%8,(row+1)*8+(i+1)%8,(row+1)*8+i) for i in range(8)])
    faces.append(list(range((len(rows)-1)*8, len(rows)*8)))
    mesh(name, points, faces, mat, projected=projected)


def tower(name, x, base, tip, sign, width):
    length = tip-base
    rows = [(base, width, .76), (base+length*.18, width*.98, .84),
            (base+length*.62, width*.64, .66), (base+length*.84, width*.44, .44),
            (tip, width*.18, .23)]
    tapered_spine(name+'.core', rows, x, sign, mat=DARK)
    for side in [-1, 1]:
        rib_x = x+side*width*.41
        tapered_spine(name+f'.rib.{side}', [(base-.12,width*.19,.68),
                       (base+length*.52,width*.17,.75),
                       (base+length*.85,width*.09,.31)], rib_x, sign, mat=STEEL, projected=False)
        beam(name+f'.feed.{side}', [rib_x,sign*(base+.14),.49],
             [rib_x,sign*(tip-.28),.28], .028, BRASS, 6)
        beam(name+f'.antenna.{side}', [x+side*width*.16,sign*(tip-.25),0],
             [x+side*width*.16,sign*(tip+.54+(side+1)*.07),0], .018, STEEL, 6, .008)
        box(name+f'.antenna-lamp.{side}', [x+side*width*.16,sign*(tip+.54+(side+1)*.07),0],
            [.025,.04,.025], AMBER)
    # Repeated collars, radiator slots, a small reactor and mirrored back details.
    for k in range(5):
        y = base+length*(.12+k*.15)
        w = width*(.95-k*.12)
        box(name+f'.collar.{k}', [x,sign*y,0], [w+.08,.09,.90-k*.085], BRASS)
        for zsign in [-1, 1]:
            box(name+f'.vent.{k}.{zsign}', [x,sign*(y+.17),zsign*(.43-k*.035)],
                [w*.62,.12,.045], BLACK)
            box(name+f'.window.{k}.{zsign}', [x+w*.33,sign*(y+.16),zsign*(.464-k*.035)],
                [.035,.13,.025], AMBER)
    for zsign in [-1, 1]:
        box(name+f'.reactor-frame.{zsign}', [x,sign*(base+length*.31),zsign*.46],
            [width*.33,length*.26,.13], STEEL)
        box(name+f'.reactor.{zsign}', [x,sign*(base+length*.31),zsign*.54],
            [width*.19,length*.23,.035], BLUE)
    beam(name+'.central-aerial', [x,sign*(tip-.10),0], [x,sign*(tip+.38),0], .025, BRASS, 6, .008)


def build_geometry():
    annulus('ring.main-beveled-structure', [(3.70,-.36),(3.77,-.57),(4.91,-.57),
        (5.16,-.36),(5.21,-.18),(5.21,.18),(5.16,.36),(4.91,.57),
        (3.77,.57),(3.70,.36)], DARK, projected=True)
    for zsign in [-1, 1]:
        z = zsign*.59
        ring(f'ring.outer-track.{zsign}', 4.98, 5.10, z-.075, z+.075, BRASS)
        ring(f'ring.inner-track.{zsign}', 3.77, 3.85, z-.045, z+.045, STEEL)
        ring(f'ring.outer-recess.{zsign}', 4.78, 4.91, z-.04, z+.04, BLACK)
        ring(f'ring.inner-brass-rail.{zsign}', 3.94, 3.99, z-.045, z+.045, BRASS)
        for i in range(48):
            angle = TAU*i/48
            gap = .012
            ring(f'ring.armor.{zsign}.{i:02}',4.05,4.75,z-.025,z+.065,STEEL,
                 start=angle+gap,sweep=TAU/48-gap*2,segments=3,projected=True)
            # Small raised radial straps and individual amber running lights.
            a = angle+.045
            box(f'ring.strap.{zsign}.{i}', [4.84*math.cos(a),4.84*math.sin(a),zsign*.69],
                [.31,.055,.06], BRASS, angle=a)
            box(f'ring.marker.{zsign}.{i}', [4.93*math.cos(a),4.93*math.sin(a),zsign*.74],
                [.055,.035,.025], AMBER, angle=a)
            if i % 2 == 0:
                for r in [4.13,4.61]:
                    box(f'ring.fastener.{zsign}.{i}.{r}', [r*math.cos(a),r*math.sin(a),zsign*.72],
                        [.055,.06,.045], DARK, angle=a)
    # An axial truss makes the ring a genuine 3D structure from edge-on views.
    for i in range(64):
        a = TAU*i/64
        box(f'ring.axial-rib.{i}', [5.17*math.cos(a),5.17*math.sin(a),0],
            [.18,.06,1.03], STEEL if i%4==0 else DARK, angle=a)
        if i % 4 == 0:
            beam(f'ring.outer-pipe.{i}', [5.23*math.cos(a),5.23*math.sin(a),-.47],
                 [5.23*math.cos(a),5.23*math.sin(a),.47], .045, BRASS, 6)
    for sign in [-1, 1]:
        tower(f'tower.central.{sign}', 0, 4.90, 7.97, sign, .76)
        for xsign in [-1, 1]:
            tower(f'tower.flank.{xsign}.{sign}', xsign*4.39, .87, 5.63, sign, .63)
    # Two long, multi-rail horizontal docking spurs.
    for sign in [-1, 1]:
        rows = [(4.60,1.00,1.12),(5.28,.86,1.02),(6.72,.59,.63),(7.91,.34,.40),(8.55,.12,.20)]
        tapered_spine(f'dock.{sign}.spine', rows, y_sign=sign, horizontal=True)
        for yoff in [-.30,.30]:
            tapered_spine(f'dock.{sign}.rail.{yoff}', [(5.1,.17,.52),(7.52,.14,.38),(8.16,.07,.21)],
                         x=yoff,y_sign=sign,horizontal=True,mat=STEEL,projected=False)
            for zsign in [-1,1]:
                beam(f'dock.{sign}.conduit.{yoff}.{zsign}',[sign*5.13,yoff,zsign*.36],
                     [sign*7.93,yoff*.46,zsign*.22],.025,BRASS,6)
        for j in range(7):
            x = sign*(5.35+j*.37)
            width = .62-j*.055
            box(f'dock.{sign}.crossbrace.{j}',[x,0,0],[.075,width,.71-j*.065],BRASS)
            for zsign in [-1,1]:
                box(f'dock.{sign}.window.{j}.{zsign}',[x,.13,zsign*(.38-j*.034)],
                    [.12,.038,.03],AMBER)
        for ysign in [-1,1]:
            box(f'dock.{sign}.landing-pad.{ysign}',[sign*7.15,ysign*.36,0],[.49,.09,.26],DARK)
            beam(f'dock.{sign}.beacon.{ysign}',[sign*7.15,ysign*.38,0],
                 [sign*7.15,ysign*.61,0],.027,STEEL,6)
    # Four emitter bridges straddle the inner lip; the passage stays open.
    for k in range(4):
        a = k*TAU/4
        radial = np.array([math.cos(a),math.sin(a),0])
        box(f'emitter.{k}.housing',radial*4.40,[1.56,.65,1.42],DARK,angle=a,projected=True)
        for sign in [-1,1]:
            box(f'emitter.{k}.face.{sign}',radial*4.34+[0,0,sign*.76],
                [1.28,.36,.12],STEEL,angle=a)
            box(f'emitter.{k}.cell.{sign}',radial*4.31+[0,0,sign*.84],
                [1.17,.105,.035],BLUE,angle=a)
            tangent = np.array([-math.sin(a),math.cos(a),0])
            for side in [-1,1]:
                box(f'emitter.{k}.guard.{sign}.{side}',radial*4.35+tangent*side*.26+[0,0,sign*.80],
                    [1.38,.065,.13],DARK,angle=a)
        box(f'emitter.{k}.aperture-tip',radial*3.66,[.11,.36,.48],WHITE,angle=a)
    ring('field.containment-lip',3.665,3.707,-.13,.13,WHITE,
         assembly='containment-field',segments=192)
    # A real mesh membrane, separately toggleable and double-sided; no solid
    # floor is placed across the opening when the portal is hidden.
    segments, rings = PORTAL_PROFILE['segments'], PORTAL_PROFILE['radialRings']
    radius, depth = PORTAL_PROFILE['radius'], PORTAL_PROFILE['depth']
    vertices = [[0,0,-depth]]
    for j in range(1,rings+1):
        r = radius*j/rings
        z = -depth*(1-(r/radius)**2)
        vertices.extend([[r*math.cos(i*TAU/segments),r*math.sin(i*TAU/segments),z] for i in range(segments)])
    faces = [(0,1+i,1+(i+1)%segments) for i in range(segments)]
    for j in range(rings-1):
        row,nxt=1+j*segments,1+(j+1)*segments
        faces.extend([(row+i,nxt+i,nxt+(i+1)%segments,row+(i+1)%segments) for i in range(segments)])
    mesh('wormhole.energy-membrane',vertices,faces,PORTAL,'wormhole',closed=False)


def export():
    merged = {}
    for part in PARTS:
        for mat,g in part['groups'].items():
            group = merged.setdefault((part['assembly'],mat),dict(position=[],normal=[],uv=[],indices=[],components=[]))
            offset = len(group['position'])
            group['components'].append(dict(name=part['name'],indexStart=len(group['indices']),
                                           indexCount=len(g['indices']),closed=part['closed']))
            for attr in ['position','normal','uv']:
                group[attr].extend(g[attr])
            group['indices'].extend(i+offset for i in g['indices'])
    binary=bytearray()
    doc=dict(asset={'version':'2.0','generator':'Stellar Syndicates reference-guided NumPy mesh builder'},
             scene=0,scenes=[{'nodes':[0]}],nodes=[{'name':'Wormhole Hub','children':[]}],
             meshes=[],materials=MATERIALS,buffers=[{}],bufferViews=[],accessors=[],images=[],textures=[],
             samplers=[{'magFilter':9729,'minFilter':9987,'wrapS':33071,'wrapT':33071}],
             extensionsUsed=['KHR_materials_unlit'])

    def view(raw,target=None):
        binary.extend(b'\0'*(-len(binary)%4))
        entry=dict(buffer=0,byteOffset=len(binary),byteLength=len(raw))
        binary.extend(raw)
        if target:
            entry['target']=target
        doc['bufferViews'].append(entry)
        return len(doc['bufferViews'])-1

    def accessor(values,component,kind,target=None,bounds=False):
        array=np.asarray(values,dtype={5126:'<f4',5123:'<u2',5125:'<u4'}[component])
        entry=dict(bufferView=view(array.tobytes(),target),componentType=component,count=len(array),type=kind)
        if bounds:
            entry.update(min=array.min(axis=0).reshape(-1).tolist(),max=array.max(axis=0).reshape(-1).tolist())
        doc['accessors'].append(entry)
        return len(doc['accessors'])-1

    for name in TEXTURES:
        doc['images'].append(dict(name=name,mimeType='image/png',bufferView=view((ROOT/'textures'/name).read_bytes())))
        doc['textures'].append(dict(source=len(doc['images'])-1,sampler=0))
    assemblies={}
    all_positions=[]
    triangles=0
    for (assembly,mat),g in merged.items():
        if assembly not in assemblies:
            assemblies[assembly]=len(doc['nodes'])
            doc['nodes'].append(dict(name=assembly,children=[]))
            doc['nodes'][0]['children'].append(assemblies[assembly])
        packed=np.column_stack([g['position'],g['normal'],g['uv']]).astype('<f4')
        unique,inverse=np.unique(packed,axis=0,return_inverse=True)
        indices=inverse[g['indices']]
        primitive=dict(attributes={
            'POSITION':accessor(unique[:,:3],5126,'VEC3',34962,True),
            'NORMAL':accessor(unique[:,3:6],5126,'VEC3',34962),
            'TEXCOORD_0':accessor(unique[:,6:8],5126,'VEC2',34962)},
            indices=accessor(indices,5123 if len(unique)<65536 else 5125,'SCALAR',34963),material=mat)
        if mat == PORTAL:
            # A baked vertex tint gives the recessed center depth in generic
            # glTF viewers, while the original galaxy texture stays unchanged.
            r = np.linalg.norm(unique[:,:2],axis=1)/PORTAL_PROFILE['radius']
            t = np.clip((r-.08)/.82,0,1)
            fade = (t*t*(3-2*t))[:,None]
            color = np.asarray(PORTAL_PROFILE['coreTint'])*(1-fade)+fade
            primitive['attributes']['COLOR_0'] = accessor(color,5126,'VEC3',34962)
        name=assembly+' / '+MATERIALS[mat]['name']
        doc['meshes'].append(dict(name=name,primitives=[primitive],extras={'sourceComponents':g['components']}))
        doc['nodes'].append(dict(name=name,mesh=len(doc['meshes'])-1))
        doc['nodes'][assemblies[assembly]]['children'].append(len(doc['nodes'])-1)
        all_positions.extend(unique[:,:3])
        triangles+=len(indices)//3
    times=accessor([0,1,2,3],5126,'SCALAR',bounds=True)
    scales=accessor([[1,1,1],[.8,.8,1],[.35,.35,1],[0,0,1]],5126,'VEC3')
    doc['animations']=[dict(name='PortalClose',samplers=[{'input':times,'output':scales,'interpolation':'LINEAR'}],
        channels=[{'sampler':0,'target':{'node':assemblies['wormhole'],'path':'scale'}}])]
    doc['buffers'][0]['byteLength']=len(binary)
    encoded=json.dumps(doc,separators=(',',':')).encode()
    encoded+=b' '*(-len(encoded)%4)
    binary.extend(b'\0'*(-len(binary)%4))
    output=struct.pack('<4sII',b'glTF',2,28+len(encoded)+len(binary))
    output+=struct.pack('<I4s',len(encoded),b'JSON')+encoded
    output+=struct.pack('<I4s',len(binary),b'BIN\0')+binary
    (ROOT/'wormhole-hub.glb').write_bytes(output)
    positions=np.asarray(all_positions)
    info=dict(name='Wormhole Hub',triangles=triangles,renderSections=len(merged),
              sourceComponents=len(PARTS),materials=len(MATERIALS),embeddedTextures=len(TEXTURES),
              bounds={'min':positions.min(axis=0).tolist(),'max':positions.max(axis=0).tolist()},
              dimensionsXYZ=np.ptp(positions,axis=0).round(4).tolist(),
              ringOuterDiameter=10.42,apertureDiameter=7.33,clearPassageDiameter=7.21,
              portalProfile=PORTAL_PROFILE,
              axes='Y up; ring in XY; front +Z; passage along Z',
              units='Relative modeling units; physical size unspecified',
              assemblies=list(assemblies),animations=['PortalClose'],
              construction='Reference-guided procedural polygon mesh, not a photogrammetry scan',
              texturing='Unmodified front image projected onto armor and portal; locally unwrapped project metal and reactor textures on remaining surfaces',
              sourceReference='reference-sheet.png',
              note='Front artwork defines circular proportions; depth and rear details are interpreted from the multi-view concept. Hardware is merged by material; OBJ keeps named components.')
    (ROOT/'model-info.json').write_text(json.dumps(info,indent=2)+'\n')
    lines=['# Wormhole Hub; Y up; +Z front','mtllib wormhole-hub.mtl']
    offset=1
    for part in PARTS:
        lines.append('o '+part['name'])
        for mat,g in part['groups'].items():
            lines.append('usemtl material_'+str(mat))
            for p in g['position']:
                lines.append('v '+' '.join(f'{v:.7f}' for v in p))
            for u,v in g['uv']:
                lines.append(f'vt {u:.8f} {1-v:.8f}')
            for n in g['normal']:
                lines.append('vn '+' '.join(f'{v:.7f}' for v in n))
            for j in range(0,len(g['indices']),3):
                lines.append('f '+' '.join(f'{i+offset}/{i+offset}/{i+offset}' for i in g['indices'][j:j+3]))
            offset+=len(g['position'])
    (ROOT/'wormhole-hub.obj').write_text('\n'.join(lines)+'\n')
    mtl=[]
    for i,mat in enumerate(MATERIALS):
        pbr=mat['pbrMetallicRoughness']
        mtl.extend([f'newmtl material_{i}','Kd '+' '.join(map(str,pbr['baseColorFactor'][:3])),
                    'Ks 0.3 0.3 0.3','Ns 48',f'Pm {pbr["metallicFactor"]}',f'Pr {pbr["roughnessFactor"]}'])
        if 'baseColorTexture' in pbr:
            mtl.append('map_Kd textures/'+TEXTURES[pbr['baseColorTexture']['index']])
        if 'emissiveFactor' in mat:
            mtl.append('Ke '+' '.join(map(str,mat['emissiveFactor'])))
        if 'emissiveTexture' in mat:
            mtl.append('map_Ke textures/'+TEXTURES[mat['emissiveTexture']['index']])
        mtl.append('')
    (ROOT/'wormhole-hub.mtl').write_text('\n'.join(mtl))
    print(json.dumps(info,indent=2),flush=True)


if __name__=='__main__':
    prepare_inputs()
    build_geometry()
    export()
