"""Inspect the exported GLB, independent of the geometry builder."""
from pathlib import Path
from collections import Counter, defaultdict
import json
import numpy as np
from render_core import Model, render

ROOT=Path(__file__).resolve().parent


def validate(model):
    components=defaultdict(list)
    closed={}
    triangles=0
    min_area=float('inf')
    min_uv=float('inf')
    hardware=[]
    for p in model.primitives:
        assert all(np.isfinite(p[k]).all() for k in ['p','n','uv'])
        assert p['i'].min()>=0 and p['i'].max()<len(p['p'])
        assert np.abs(np.linalg.norm(p['n'],axis=1)-1).max()<1e-4
        vertices=p['p'][p['i']].astype(float)
        cross=np.cross(vertices[:,1]-vertices[:,0],vertices[:,2]-vertices[:,0])
        areas=np.linalg.norm(cross,axis=1)*.5
        assert areas.min()>1e-10,(p['name'],'degenerate face')
        assert (cross*p['n'][p['i']].mean(axis=1)).sum(axis=1).min()>-1e-7,(p['name'],'winding')
        min_area=min(min_area,float(areas.min()))
        assert p['uv'].min()>=0 and p['uv'].max()<=1
        uv=p['uv'][p['i']].astype(float)
        a,b=uv[:,1]-uv[:,0],uv[:,2]-uv[:,0]
        uv_area=np.abs(a[:,0]*b[:,1]-a[:,1]*b[:,0])*.5
        assert uv_area.min()>1e-12,(p['name'],'collapsed texture coordinates')
        min_uv=min(min_uv,float(uv_area.min()))
        if p['name'].startswith('station-hardware'):
            hardware.extend(p['p'])
        for c in p['components']:
            start,end=c['indexStart']//3,(c['indexStart']+c['indexCount'])//3
            components[c['name']].extend(vertices[start:end])
            closed[c['name']]=c['closed']
        triangles+=len(vertices)
    solid_count=0
    volumes={}
    for name,faces in components.items():
        edges=Counter()
        for triangle in faces:
            points=[tuple(np.round(v,6)) for v in triangle]
            for a,b in zip(points,points[1:]+points[:1]):
                edges[tuple(sorted([a,b]))]+=1
        if closed[name]:
            assert all(n==2 for n in edges.values()),(name,'open solid',Counter(edges.values()))
            v=np.asarray(faces)
            volume=float((v[:,0]*np.cross(v[:,1],v[:,2])).sum()/6)
            assert volume>1e-10,(name,'nonpositive volume',volume)
            solid_count+=1
            volumes[name]=volume
        else:
            assert name=='wormhole.energy-membrane'
            assert all(n in [1,2] for n in edges.values())
            assert sum(n==1 for n in edges.values())==128
    hardware=np.asarray(hardware)
    minimum_radius=float(np.linalg.norm(hardware[:,:2],axis=1).min())
    assert minimum_radius>3.55,'Hardware occludes the passage'
    assert len([name for name in components if name.startswith('tower.') and name.endswith('.core')])==6
    assert len([name for name in components if name.startswith('dock.') and name.endswith('.spine')])==2
    portal=next(p for p in model.primitives if p['name'].startswith('wormhole'))
    portal_radius=np.linalg.norm(portal['p'][:,:2],axis=1)
    measured_depth=float(np.ptp(portal['p'][:,2]))
    assert measured_depth>=1.5,'The portal must have a visibly recessed center'
    assert np.abs(portal['p'][portal_radius>3.684,2]).max()<1e-6,'Rim must stay anchored to the ring'
    assert np.isfinite(portal['color']).all() and portal['color'].min()>=0 and portal['color'].max()<=1
    assert portal['color'][portal_radius<.1].mean()<portal['color'][portal_radius>3.6].mean()
    result=dict(triangles=triangles,closedSolidComponents=solid_count,
                intentionalOpenSurfaces=['wormhole.energy-membrane'],finiteAttributes=True,
                unitNormals=True,consistentWinding=True,nondegenerateFaces=True,
                noncollapsedUVs=True,embeddedTextures=len(model.images),
                minimumTriangleArea=min_area,minimumUVArea=min_uv,
                minimumHardwareRadius=minimum_radius,clearPassageDiameter=minimum_radius*2,
                towerSpires=6,dockingArms=2,
                portalDepth=measured_depth,anchoredPortalRim=True,bakedPortalDepthTint=True,
                method='Independent checks on exported GLB buffers and per-component welded topology')
    (ROOT/'geometry-validation.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result,indent=2),flush=True)


if __name__=='__main__':
    model=Model(ROOT/'wormhole-hub.glb')
    validate(model)
    out=ROOT/'previews'
    out.mkdir(exist_ok=True)
    views={
        'hero':((.65,.36,1),(0,1,0)),
        'front':((0,0,1),(0,1,0)),
        'rear-quarter':((-.62,.32,-1),(0,1,0)),
        'back':((0,0,-1),(0,1,0)),
        'left':((-1,0,0),(0,1,0)),
        'right':((1,0,0),(0,1,0)),
        'top':((0,1,0),(0,0,-1)),
        'bottom':((0,-1,0),(0,0,1)),
    }
    for name,(camera,up) in views.items():
        render(model,camera,up,width=1200,height=1100).save(out/f'{name}.png')
        print('Rendered',name,flush=True)
    model.primitives=[p for p in model.primitives if not p['name'].startswith(('wormhole','containment-field'))]
    render(model,(.65,.36,1),width=1200,height=1100).save(out/'portal-off.png')
    print('Rendered portal-off',flush=True)
