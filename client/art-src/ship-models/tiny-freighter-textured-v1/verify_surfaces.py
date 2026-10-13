"""Inspect the exported artifact, including underside coverage and UV area."""
from pathlib import Path
import json
import struct
import numpy as np

ROOT=Path(__file__).resolve().parent
raw=(ROOT/'tiny-freighter.glb').read_bytes()
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
required=('cargo.','bridge | faceted','bridge.port | sensor','bridge.starboard | sensor',
          'hull | continuous','stern | central','engine.port | stepped','engine.starboard | stepped')
for m in doc['meshes']:
    for p in m['primitives']:
        report['renderSections']+=1
        positions=read(p['attributes']['POSITION'])
        normals=read(p['attributes']['NORMAL'])
        uv=read(p['attributes']['TEXCOORD_0'])
        indices=read(p['indices']).flatten().reshape(-1,3)
        tri=positions[indices];tex=uv[indices]
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
            if not name.startswith(required):continue
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
                inside=(px[:,:,0]>=511)&(px[:,:,0]<=906)&(px[:,:,1]>=55)&(px[:,:,1]<=820)
                entry['bottomViewUVs']+=int(np.count_nonzero(inside.all(axis=1)))
for img in doc['images']:
    view=doc['bufferViews'][img['bufferView']]
    embedded=binary[view['byteOffset']:view['byteOffset']+view['byteLength']]
    report['embeddedImagesMatch']&=embedded==(ROOT/'textures'/img['name']).read_bytes()
report['engineSockets']=[n['name'] for n in doc['nodes'] if n['name'].startswith('socket.engine.')]
expected={
    'hull | continuous structural keel',
    'cargo.1 | beveled pressure container','cargo.2 | beveled pressure container',
    'cargo.1 | ventral access hatch','cargo.2 | ventral access hatch',
    'bridge | faceted forward pressure cabin',
    'bridge.port | sensor and maneuvering fairing',
    'bridge.starboard | sensor and maneuvering fairing',
    'stern | central drive-service fairing',
    'engine.port | stepped armored engine casing',
    'engine.starboard | stepped armored engine casing',
}
report['missingUndersideComponents']=sorted(expected-set(report['bottomComponents']))
report['fullyTexturedUnderside']=not report['missingUndersideComponents'] and all(
    v['triangles']==v['textured']==v['validUVArea']==v['bottomViewUVs']
    for name,v in report['bottomComponents'].items()
    if 'running-light' not in name and 'running strip' not in name
)
report['passed']=not any(report[k] for k in ['zeroAreaGeometry','zeroAreaTexturedUVs','nonFiniteValues']) and report['fullyTexturedUnderside'] and report['embeddedImagesMatch'] and len(report['engineSockets'])==2
(ROOT/'surface-verification.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2))
if not report['passed']:raise SystemExit(1)
