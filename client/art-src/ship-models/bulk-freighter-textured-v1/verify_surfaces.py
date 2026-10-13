"""Inspect the exported artifact, including underside coverage and UV area."""
from pathlib import Path
import json
import struct
import numpy as np

ROOT=Path(__file__).resolve().parent
raw=(ROOT/'bulk-freighter.glb').read_bytes()
assert raw[:4]==b'glTF'
json_length=struct.unpack_from('<I',raw,12)[0]
doc=json.loads(raw[20:20+json_length])
binary=raw[28+json_length:]
def read(index):
    a=doc['accessors'][index];view=doc['bufferViews'][a['bufferView']]
    offset=view.get('byteOffset',0)+a.get('byteOffset',0)
    width={'VEC3':3,'VEC2':2,'SCALAR':1}[a['type']]
    dtype={5126:'<f4',5125:'<u4',5123:'<u2'}[a['componentType']]
    return np.frombuffer(binary,dtype=dtype,count=a['count']*width,offset=offset).reshape(a['count'],width)

report={'triangles':0,'renderSections':0,'zeroAreaGeometry':0,
        'zeroAreaTexturedUVs':0,'nonFiniteValues':0,'bottomComponents':{},
        'engineSockets':[],'embeddedImagesMatch':True}
expected={'bridge | faceted command pod','bridge | ventral access hatch',
    'bridge.port | sensor fairing','bridge.starboard | sensor fairing',
    'stern | central reactor-service fairing','drive | eight-engine support'}
for row in range(1,5):
    expected.update({f'spine.{row} | dorsal distribution housing',f'spine.{row} | ventral service housing'})
    for side in ('port','starboard'):
        prefix=f'cargo.{side}.{row}'
        expected.update({prefix+' | mineral silo',prefix+' | rectangular cargo cage',prefix+' | cage ventral inspection panel'})
        for hx in ((63,155) if side=='port' else (299,391)):
            expected.add(prefix+f' | silo sump hatch {hx}')
for side in ('port','starboard'):
    for i in range(1,4):expected.add(f'drive.{side}.{i} | radiator housing')
for i in range(1,9):expected.add(f'engine.{i} | armored engine casing')
cargo_geometry={}
footprints=[]
for m in doc['meshes']:
    for p in m['primitives']:
        report['renderSections']+=1
        positions=read(p['attributes']['POSITION'])
        normals=read(p['attributes']['NORMAL'])
        uv=read(p['attributes']['TEXCOORD_0'])
        indices=read(p['indices']).flatten().reshape(-1,3)
        tri=positions[indices];tex=uv[indices]
        footprints.append(tri[:,:,[0,2]])
        report['triangles']+=len(tri)
        report['nonFiniteValues']+=int(sum(np.count_nonzero(~np.isfinite(a)) for a in (positions,normals,uv)))
        cross=np.cross(tri[:,1]-tri[:,0],tri[:,2]-tri[:,0])
        area=np.linalg.norm(cross,axis=1)
        report['zeroAreaGeometry']+=int(np.count_nonzero(area<1e-9))
        normal=cross/np.maximum(area[:,None],1e-20)
        uv_a=tex[:,1]-tex[:,0];uv_b=tex[:,2]-tex[:,0]
        uv_area=np.abs(uv_a[:,0]*uv_b[:,1]-uv_a[:,1]*uv_b[:,0])
        mat=doc['materials'][p['material']]
        textured='baseColorTexture' in mat['pbrMetallicRoughness']
        if textured:report['zeroAreaTexturedUVs']+=int(np.count_nonzero(uv_area<1e-12))
        for c in m.get('extras',{}).get('sourceComponents',[]):
            name=c['name']
            start=c['indexStart']//3;end=start+c['indexCount']//3
            if name.endswith((' | mineral silo',' | rectangular cargo cage')):
                cargo_geometry.setdefault(name,[]).extend(tri[start:end])
            if name not in expected:continue
            start=c['indexStart']//3;end=start+c['indexCount']//3
            selected=np.arange(start,end)[normal[start:end,1]<-.35]
            if not len(selected):continue
            entry=report['bottomComponents'].setdefault(name,{'triangles':0,'textured':0,'validUVArea':0,'bottomViewUVs':0})
            entry['triangles']+=len(selected)
            if textured:
                entry['textured']+=len(selected)
                entry['validUVArea']+=int(np.count_nonzero(uv_area[selected]>1e-12))
                # All source pixels must lie in the dedicated BOTTOM drawing.
                px=tex[selected]*2048
                inside=(px[:,:,0]>=471)&(px[:,:,0]<=891)&(px[:,:,1]>=18)&(px[:,:,1]<=834)
                entry['bottomViewUVs']+=int(np.count_nonzero(inside.all(axis=1)))
for img in doc['images']:
    view=doc['bufferViews'][img['bufferView']]
    embedded=binary[view['byteOffset']:view['byteOffset']+view['byteLength']]
    report['embeddedImagesMatch']&=embedded==(ROOT/'textures'/img['name']).read_bytes()
report['engineSockets']=[n['name'] for n in doc['nodes'] if n['name'].startswith('socket.engine.')]
report['missingUndersideComponents']=sorted(expected-set(report['bottomComponents']))
report['fullyTexturedUnderside']=not report['missingUndersideComponents'] and all(
    v['triangles']==v['textured']==v['validUVArea']==v['bottomViewUVs']
    for name,v in report['bottomComponents'].items()
    if 'running-light' not in name and 'running strip' not in name
)
# Rays pass through the negative space beside each cargo arm and through
# selected spine-web openings. A filled-in floor would intersect these rays.
footprint=np.concatenate(footprints)
a=footprint[:,0];b=footprint[:,1];c=footprint[:,2]
def cross2(u,v):return u[:,0]*v[:,1]-u[:,1]*v[:,0]
area=cross2(b-a,c-a)
rays=[]
points=[(px,py) for px in (200,254) for py in (167,210,307,350,448,491,589,632)]
points += [(230,178),(227,300),(227,440),(230,597)]
for px,py in points:
    x=(px-227)/40;z=(py-430)/40;p=np.array([x,z])
    with np.errstate(divide='ignore',invalid='ignore'):
        u=cross2(b-a,p-a)/area;v=cross2(p-a,c-a)/area
        hits=(np.abs(area)>1e-10)&(u>=-1e-7)&(v>=-1e-7)&(u+v<=1+1e-7)
    rays.append({'referencePixel':[px,py],'surfaceHits':int(hits.sum())})
report['openTrussRays']=rays
report['trussGapsOpen']=all(r['surfaceHits']==0 for r in rays)
# Cargo modules must be closed across UV/material seams, not textured cards.
from collections import Counter
closed={}
for name,rows in cargo_geometry.items():
    triangles=np.array(rows);edges=Counter()
    for tri in triangles:
        points=[tuple(np.round(p,6)) for p in tri]
        for a,b in zip(points,points[1:]+points[:1]):edges[tuple(sorted((a,b)))]+=1
    volume=float(np.sum(np.einsum('ij,ij->i',triangles[:,0],np.cross(triangles[:,1],triangles[:,2])))/6)
    closed[name]={'closedManifold':all(n==2 for n in edges.values()),'signedVolume':volume}
report['cargoModules']=closed
report['sixteenSolidCargoModules']=len(closed)==16 and all(v['closedManifold'] and v['signedVolume']>0 for v in closed.values())
report['cargoSockets']=[n['name'] for n in doc['nodes'] if n['name'].startswith('socket.cargo.')]
engine_positions=np.array([n['translation'] for n in doc['nodes'] if n['name'].startswith('socket.engine.')])
report['eightEnginesInOneRow']=len(engine_positions)==8 and len(set(engine_positions[:,0]))==8 and float(np.ptp(engine_positions[:,1]))<1e-8 and float(np.ptp(engine_positions[:,2]))<1e-8
report['passed']=not any(report[k] for k in ['zeroAreaGeometry','zeroAreaTexturedUVs','nonFiniteValues']) and report['fullyTexturedUnderside'] and report['embeddedImagesMatch'] and report['eightEnginesInOneRow'] and report['trussGapsOpen'] and report['sixteenSolidCargoModules'] and len(report['cargoSockets'])==16
(ROOT/'surface-verification.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2))
if not report['passed']:raise SystemExit(1)
