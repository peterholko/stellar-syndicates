"""Check all authored surfaces, UV projections, sockets and embedded textures."""
from pathlib import Path
import json,struct
import numpy as np
from scaffold import SPECS,ROOT,MODELS

def inspect(ship):
    folder=MODELS/(ship+'-textured-v1');spec=json.loads((folder/'model-spec.json').read_text())
    raw=(folder/(ship+'.glb')).read_bytes();n=struct.unpack_from('<I',raw,12)[0]
    doc=json.loads(raw[20:20+n]);binary=raw[28+n:]
    def attribute(i):
        a=doc['accessors'][i];v=doc['bufferViews'][a['bufferView']]
        return np.frombuffer(binary,dtype={5126:'<f4',5123:'<u2',5125:'<u4'}[a['componentType']],
              count=a['count']*{'VEC3':3,'VEC2':2,'SCALAR':1}[a['type']],offset=v.get('byteOffset',0)+a.get('byteOffset',0)).reshape(a['count'],-1)
    problems=[];textured=ventral=triangles=0
    for m in doc['meshes']:
        for p in m['primitives']:
            pos=attribute(p['attributes']['POSITION']);uv=attribute(p['attributes']['TEXCOORD_0']);ids=attribute(p['indices']).reshape(-1,3)
            material=doc['materials'][p['material']];triangles+=len(ids)
            poly=pos[ids];area=np.linalg.norm(np.cross(poly[:,1]-poly[:,0],poly[:,2]-poly[:,0]),axis=1)
            bad=area<1e-9
            if 'baseColorTexture' in material['pbrMetallicRoughness']:
                textured+=len(ids);t=uv[ids];a=t[:,1]-t[:,0];b=t[:,2]-t[:,0]
                bad|=np.abs(a[:,0]*b[:,1]-a[:,1]*b[:,0])<1e-12
                if material['name'].startswith('Ventral'):
                    ventral+=len(ids);x0,y0,x1,y1=spec['bottom'];px=t*2048
                    inside=(px[:,:,0]>=x0-.01)&(px[:,:,0]<=x1+.01)&(px[:,:,1]>=y0-.01)&(px[:,:,1]<=y1+.01)
                    bad|=~inside.all(axis=1)
            if bad.any():
                for c in m['extras']['sourceComponents']:
                    section=bad[c['indexStart']//3:(c['indexStart']+c['indexCount'])//3]
                    if section.any():problems.append({'component':c['name'],'material':material['name'],'badTriangles':int(section.sum())})
    validation=json.loads((folder/'validation.json').read_text())['issues']
    names=[n.get('name','') for n in doc['nodes']]
    checks={'allSurfacesAndTexturedUVsHaveArea':not problems,'ventralProjectionPresent':ventral>0,
            'engineCount':sum(n.startswith('socket.engine.') for n in names)==spec['engines'],
            'turretCount':sum(n.startswith('socket.turret.') for n in names)==spec['turrets'],
            'threeEmbeddedTextures':len(doc['images'])==3,
            'gltfValidatorClean':validation['numErrors']==validation['numWarnings']==0}
    report={'ship':ship,'triangles':triangles,'texturedTriangles':textured,'ventralTriangles':ventral,'checks':checks,'problems':problems,'passed':all(checks.values())}
    (folder/'surface-verification.json').write_text(json.dumps(report,indent=2)+'\n')
    return report

if __name__=='__main__':
    results=[inspect(ship) for ship in SPECS]
    (ROOT/'surface-verification.json').write_text(json.dumps(results,indent=2)+'\n')
    print(json.dumps([{'ship':r['ship'],'passed':r['passed'],'problems':r['problems']} for r in results],indent=2))
    if not all(r['passed'] for r in results):raise SystemExit(1)
