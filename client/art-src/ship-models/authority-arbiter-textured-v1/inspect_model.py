"""Inspect the exported Arbiter buffers and render the real textured model.

This does not launch a browser and is not a WebGL compatibility test.
"""
from pathlib import Path
from collections import Counter
import json
import re
import numpy as np
from PIL import Image, ImageDraw, ImageFont
from render_core import Model, render, bilinear

ROOT=Path(__file__).resolve().parent


def ray_surface(model,xy,front=False):
    """Nearest textured surface seen along the model's longitudinal axis."""
    depth=np.inf if front else -np.inf
    visible=None;component_name=None
    for prim in model.primitives:
        triangles=prim['p'][prim['i']];projected=triangles[:,:,:2]
        a=projected[:,1]-projected[:,0];b=projected[:,2]-projected[:,0];rel=xy-projected[:,0]
        determinant=a[:,0]*b[:,1]-a[:,1]*b[:,0]
        stable=abs(determinant)>1e-9;safe=np.where(stable,determinant,1)
        u=(rel[:,0]*b[:,1]-rel[:,1]*b[:,0])/safe
        v=(a[:,0]*rel[:,1]-a[:,1]*rel[:,0])/safe
        hits=np.flatnonzero(stable&(u>=-1e-7)&(v>=-1e-7)&(u+v<=1+1e-7))
        for i in hits:
            bary=np.array([1-u[i]-v[i],u[i],v[i]])
            z=float(bary@triangles[i,:,2])
            if (z<depth if front else z>depth):
                depth=z;uv=bary@prim['uv'][prim['i'][i]]
                texture=model.doc['textures'][prim['m']['pbrMetallicRoughness']['baseColorTexture']['index']]['source']
                visible=bilinear(model.images[texture],uv[None])[0]
                component_name=next(c['name'] for c in prim['components'] if c['indexStart']<=i*3<c['indexStart']+c['indexCount'])
    return depth,visible,component_name


def component_buffers(model):
    buffers={}
    for prim in model.primitives:
        for part in prim['components']:
            indices=prim['i'][part['indexStart']//3:(part['indexStart']+part['indexCount'])//3]
            buffers[(prim['m']['name'],part['name'])]=np.concatenate([prim[k][indices] for k in ('p','n','uv')],axis=2)
    return buffers


def inspect():
    model=Model(ROOT/'authority-arbiter.glb')
    checks=model.validate()
    parts=set();areas=[];downward=0
    for prim in model.primitives:
        pbr=prim['m']['pbrMetallicRoughness']
        assert 'baseColorTexture' in pbr and 'metallicRoughnessTexture' in pbr
        assert 'emissiveTexture' in prim['m']
        uv=prim['uv'][prim['i']]
        a,b=uv[:,1]-uv[:,0],uv[:,2]-uv[:,0]
        uv_area=np.abs(a[:,0]*b[:,1]-a[:,1]*b[:,0])/2
        assert uv_area.min()>1e-12,(prim['name'],'collapsed UVs')
        areas.extend(uv_area.tolist())
        downward+=int(np.count_nonzero(prim['n'][prim['i']].mean(axis=1)[:,1]<-.55))
        parts.update(c['name'] for c in prim['components'])
    nodes={n['name']:n for n in model.doc['nodes']}
    parents={child:index for index,node in enumerate(model.doc['nodes']) for child in node.get('children',[])}
    roles=Counter(n.get('extras',{}).get('role','') for n in nodes.values())
    counts={
        'haloRings':int('halo.continuous-armored-ring' in parts),
        'haloStruts':len([n for n in parts if re.fullmatch(r'halo\.strut\.\d+',n)]),
        'dorsalSpines':len([n for n in parts if re.fullmatch(r'spine\.\d+\.ridge',n)]),
        'spinalLances':int('lance.spinal-barrel' in parts),
        'massDrivers':int('mass-driver.barrel' in parts),
        'accelerationRails':len([n for n in parts if re.fullmatch(r'mass-driver\.rail\.(port|starboard)',n)]),
        'heavyTwinTurrets':roles['turret-yaw'],
        'secondaryTwinTurrets':roles['secondary-turret-yaw'],
        'pairedPointDefenseMounts':roles['point-defense-yaw'],
        'gunBarrels':len([n for n in parts if re.search(r'\.barrel\.[12]$',n)]),
        'mainEngines':len([n for n in parts if re.fullmatch(r'engine\.\d+\.cylindrical-housing',n)]),
        'commandTowers':int('command.narrow-tower' in parts),
        'closedVentralHangars':int('hangar.outer-coaming' in parts),
    }
    spec=json.loads((ROOT/'model-spec.json').read_text())
    info=json.loads((ROOT/'model-info.json').read_text())
    assert counts==spec['counts'],counts
    assert len(areas)==info['triangles']==checks['triangles']
    assert spec['triangleBudget'][0]<=len(areas)<=spec['triangleBudget'][1]
    assert len(model.primitives)==info['renderSections']
    # All muzzle empties inherit their actual moving mount, at the correct
    # rest-pose position. This catches accidental world/local pivot mix-ups.
    sockets=0
    for expected in info['sockets']:
        name=expected['name'];node=nodes[name]
        assert np.allclose(model.node_world[name][:3,3],expected['translation'],atol=1e-6),name
        assert np.isclose(np.linalg.norm(node['extras']['direction']),1),name
        assembly=node['extras'].get('assembly')
        if assembly:
            index=next(i for i,n in enumerate(model.doc['nodes']) if n['name']==name)
            assert model.doc['nodes'][parents[index]]['name']==assembly,name
            assert np.allclose(model.node_world[assembly][:3,3],info['assemblies'][assembly]['pivotYUp'])
        sockets+=1
    assert sockets==73,sockets
    assert roles['turret-muzzle']==12 and roles['secondary-muzzle']==16 and roles['point-defense-muzzle']==16
    engine_positions=np.array([model.node_world[f'socket.engine.{i}'][:3,3] for i in range(1,5)])
    assert np.ptp(engine_positions[:,1])<1e-8 and np.ptp(engine_positions[:,2])<1e-8
    assert np.allclose(np.diff(engine_positions[:,0]),np.diff(engine_positions[:,0])[0])
    # Look directly into each nozzle using the actual exported triangles. The
    # nearest surface must be the cyan exhaust, not a closed housing end cap.
    visible_exhausts=[]
    for position in engine_positions:
        _,visible,name=ray_surface(model,position[:2])
        assert name.endswith('.exhaust-disc'),name
        # The aperture's hot core is nearly white; it still has a measurable
        # cyan cast, unlike the warm ceramic cap that originally covered it.
        assert visible is not None and visible[2]>visible[0]+.055 and visible[1]>visible[0]+.055,visible
        visible_exhausts.append(np.round(visible,4).tolist())
    driver_position=model.node_world['socket.weapon.mass-driver.muzzle'][:3,3]
    depth,color,part=ray_surface(model,driver_position[:2],front=True)
    assert part=='mass-driver.breech-chamber',(part,'mass-driver bore obstructed')
    assert color.max()<.4,('mass driver should have a dark mechanical bore',color)
    component_data=component_buffers(model)
    detail_points={key:np.concatenate([v[:,:,:3].reshape(-1,3) for (_,name),v in component_data.items() if name.startswith(prefix)])
                   for key,prefix in (('mass-driver','mass-driver.'),('spines','spine.'))}
    muzzle_z=min(v[:,:,2].min() for (_,name),v in component_data.items() if name=='mass-driver.barrel')
    assert depth-muzzle_z>1.8,('bore is too shallow',depth-muzzle_z)
    assert driver_position[2]<muzzle_z,('socket is behind the muzzle',driver_position)
    assert np.ptp(detail_points['mass-driver'],axis=0)[0]>2,'Mass driver is undersized'
    spine_tips=[min(v[:,:,2].min() for (_,name),v in component_data.items() if name==f'spine.{i}.ridge') for i in (1,2,3)]
    assert spine_tips[0]>spine_tips[1]>spine_tips[2],spine_tips
    assert all(sum(n.startswith(f'spine.{i}.armor-segment.') for n in parts)==4 for i in (1,2,3))
    assert not any(n.startswith('lance.') for n in parts)
    prow_front=min(v[:,:,2].min() for (_,name),v in component_data.items() if name.startswith('prow.') and name.endswith('ceramic-plate'))
    projection=max(0,prow_front-muzzle_z)
    assert projection/np.ptp(model.positions[:,2])<.025,('Weapon protrudes too far beyond pointed prow',projection)
    weapon_names=[name for name in nodes if name.startswith('socket.weapon.')]
    assert len(weapon_names)==45,len(weapon_names)
    assert not any(name.startswith(('socket.muzzle.','socket.mass-driver.')) for name in nodes)
    assert len(model.doc['materials'])==2
    assert all(image.shape[:2]==(1024,1024) for image in model.images)
    assert (ROOT/'authority-arbiter.glb').stat().st_size<3_000_000
    gold_area=total_spine_area=0
    for prim in model.primitives:
        for part in prim['components']:
            if not re.match(r'spine\.\d+\.armor-segment\.',part['name']):continue
            indices=prim['i'][part['indexStart']//3:(part['indexStart']+part['indexCount'])//3]
            p=prim['p'][indices];uv=prim['uv'][indices].mean(axis=1)
            normals=prim['n'][indices].mean(axis=1)
            upward=normals[:,1]>.45
            area=np.linalg.norm(np.cross(p[:,1]-p[:,0],p[:,2]-p[:,0]),axis=1)/2
            gold=(uv[:,0]>.5)&(uv[:,1]<.5)
            total_spine_area+=area[upward].sum();gold_area+=area[upward&gold].sum()
    assert gold_area/total_spine_area>.9
    # Four deliberately empty points between the ring and hull must be actual
    # holes, with no transparent or painted polygons filling the halo.
    openings=[]
    for x in (-5.7,5.7):
        for z in (-2.0,2.0):
            q=np.array([x,z]);occupied=False
            for prim in model.primitives:
                tri=prim['p'][prim['i']][:,:,[0,2]]
                edges=np.roll(tri,-1,axis=1)-tri;rel=q-tri
                signed=edges[:,:,0]*rel[:,:,1]-edges[:,:,1]*rel[:,:,0]
                a,b=tri[:,1]-tri[:,0],tri[:,2]-tri[:,0]
                area=a[:,0]*b[:,1]-a[:,1]*b[:,0]
                occupied|=bool(np.any(((signed>1e-7).all(axis=1)|(signed< -1e-7).all(axis=1))&(abs(area)>1e-8)))
            assert not occupied,('filled halo opening',q)
            openings.append(q.tolist())
    checks.update(texturedPhysicalTriangles=len(areas),texturedDownwardTriangles=downward,
        allPhysicalTrianglesHaveAreaUVs=True,minimumUVTriangleArea=min(areas),
        observedFeatureCounts=counts,independentGunPivots=22,weaponSocketsFollowMounts=True,
        socketCount=sockets,engineBankAligned=True,visibleCyanExhausts=visible_exhausts,openHaloSamplePointsXZ=openings,
        massDriverBoreUnobstructed=True,massDriverBoreDepth=round(float(depth-muzzle_z),5),massDriverSocketOutsideMuzzle=True,
        spineArmorSegments=[4,4,4],spineLengthsRemainStaggered=True,
        weaponSocketCount=45,weaponExtensionBeyondProw=round(float(projection),4),
        weaponExtensionFractionOfShip=round(float(projection/np.ptp(model.positions[:,2])),5),
        goldFractionOfUpwardSpineArmor=round(float(gold_area/total_spine_area),4),
        browserVerification='Not performed; browser access unavailable by saved permission. Software renders inspect actual exported triangles and embedded textures.')
    (ROOT/'surface-verification.json').write_text(json.dumps(checks,indent=2)+'\n')
    preview=ROOT/'previews';preview.mkdir(exist_ok=True)
    views=[('three-quarter',(1,.85,-1.4),(0,1,0)),
           ('rear-quarter',(-1,.80,1.3),(0,1,0)),
           ('underside',(1,-.9,-1.3),(0,1,0)),
           ('top',(0,1,0),(0,0,-1)),('bottom',(0,-1,0),(0,0,-1)),
           ('front',(0,0,-1),(0,1,0)),('back',(0,0,1),(0,1,0)),
           ('side',(-1,0,0),(0,1,0))]
    for name,camera,up in views:
        print('Rendering',name,flush=True)
        render(model,camera,up,width=1500,height=1100).save(preview/(name+'.png'))
    for key,camera in (('mass-driver',(.8,.65,-1.3)),('spines',(1,1.1,-1.25))):
        print('Rendering',key+'-detail',flush=True)
        render(model,camera,width=1500,height=1000,framing_points=detail_points[key]).save(preview/(key+'-detail.png'))
    sheet=Image.new('RGB',(1800,1452),(14,18,24));draw=ImageDraw.Draw(sheet)
    try:font=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf',19)
    except OSError:font=ImageFont.load_default()
    for i,(name,_,_) in enumerate(views):
        x=(i%3)*600;y=(i//3)*484
        im=Image.open(preview/(name+'.png')).resize((600,440),Image.Resampling.LANCZOS)
        sheet.paste(im,(x,y));draw.text((x+20,y+449),name.upper(),font=font,fill=(208,198,174))
    sheet.save(preview/'contact-sheet.png')
    print(json.dumps({k:checks[k] for k in ('triangles','closedComponents','texturedPhysicalTriangles','texturedDownwardTriangles','observedFeatureCounts','socketCount')},indent=2))


if __name__=='__main__':inspect()
