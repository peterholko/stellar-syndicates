"""Validate and render the actual exported trader GLBs, independent of builders."""
from pathlib import Path
import argparse
import hashlib
import json
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFont

ROOT=Path(__file__).resolve().parent
sys.path.insert(0,str(ROOT.parent/'mining-fleet-v1'))
from render_core import Model, render

SHIPS=['clean-slate','second-wind','small-fortune']
VIEWS=[('three-quarter',(1.2,.95,-1.45),(0,1,0)),
       ('underside',(1,-1,-1.3),(0,1,0)),
       ('top',(0,1,0),(0,0,-1)),('bottom',(0,-1,0),(0,0,-1)),
       ('front',(0,0,-1),(0,1,0)),('back',(0,0,1),(0,1,0)),
       ('side',(-1,0,0),(0,1,0))]


def clear_vertical_ray(model,x,z):
    """A genuine opening has no triangle intersecting a vertical inspection ray."""
    for primitive in model.primitives:
        tri=primitive['p'][primitive['i']][:,:,[0,2]]
        a,b,c=tri[:,0],tri[:,1],tri[:,2]
        cross=lambda u,v:u[:,0]*v[:,1]-u[:,1]*v[:,0]
        den=cross(b-a,c-a)
        usable=np.abs(den)>1e-9
        q=np.array([x,z])-a
        u=np.divide(cross(q,c-a),den,out=np.full_like(den,-100),where=usable)
        v=np.divide(cross(b-a,q),den,out=np.full_like(den,-100),where=usable)
        if np.any(usable & (u>=-1e-6) & (v>=-1e-6) & (u+v<=1+1e-6)):
            return False
    return True


def inspect(slug,do_render=True):
    folder=ROOT.parent/(slug+'-textured-v1')
    model=Model(folder/(slug+'.glb'))
    spec=json.loads((folder/'model-spec.json').read_text())
    info=json.loads((folder/'model-info.json').read_text())
    checks=model.validate()
    components=set()
    minimum_uv=1.
    downward=0
    for p in model.primitives:
        material=p['m']
        assert 'baseColorTexture' in material['pbrMetallicRoughness']
        assert 'metallicRoughnessTexture' in material['pbrMetallicRoughness']
        assert 'emissiveTexture' in material
        uv=p['uv'][p['i']]
        a,b=uv[:,1]-uv[:,0],uv[:,2]-uv[:,0]
        areas=np.abs(a[:,0]*b[:,1]-a[:,1]*b[:,0])/2
        assert areas.min()>1e-12,(slug,'collapsed UVs')
        minimum_uv=min(minimum_uv,float(areas.min()))
        downward+=int(np.count_nonzero(p['n'][p['i']].mean(axis=1)[:,1]<-.55))
        components.update(c['name'] for c in p['components'])
    counts=dict(
        cargoBays=sum(n.startswith('cargo.bay.') and n.endswith('.lid') for n in components)+int('cargo.workshop.lid' in components),
        dockingCollars=sum(n.startswith('docking.') and n.endswith('.housing') for n in components),
        mainEngines=sum(n.startswith('engine.') and n.endswith('.housing') for n in components),
        bridges=int('bridge.glazing' in components),cranes=int('crane.base' in components),
        guns=sum(n.startswith(('weapon.','gun.','turret.')) for n in components))
    assert counts==spec['counts'],(slug,counts)
    sockets={n['name']:n for n in model.doc['nodes'] if n['name'].startswith('socket.')}
    expected={'socket.engine.port':[0.,0.,1.],'socket.engine.starboard':[0.,0.,1.],
              'socket.docking.port':[-1.,0.,0.]}
    if slug=='clean-slate':expected['socket.docking.starboard']=[1.,0.,0.]
    if slug=='second-wind':expected['socket.utility.crane']=[0.,0.,1.]
    assert set(sockets)==set(expected),(slug,set(sockets))
    for name,direction in expected.items():
        assert np.allclose(sockets[name]['extras']['direction'],direction),name
        assert np.isfinite(sockets[name]['translation']).all(),name
    assert spec['triangleBudget'][0]<=checks['triangles']<=spec['triangleBudget'][1]
    assert len(model.primitives)==2
    assert all(im.shape==(2048,2048,3) for im in model.images)
    assert np.allclose(np.ptp(model.positions,axis=0),info['dimensionsXYZ'],atol=.0001)
    openings=[]
    samples={'small-fortune':[(196,610),(328,610)],'second-wind':[(278,222)]}.get(slug,[])
    for px,py in samples:
        x=(px-spec['center'][0])/38
        z=(py-spec['center'][1])/38
        assert clear_vertical_ray(model,x,z),(slug,'filled intended opening',px,py)
        openings.append(dict(referencePixel=[px,py],modelXZ=[x,z],clear=True))
    source=folder.parents[1]/spec['reference']
    assert source.read_bytes()==(folder/'textures/reference-sheet.png').read_bytes()
    for entry in json.loads((folder/'source-integrity.json').read_text())['sources']:
        assert hashlib.sha256(Path(entry['path']).read_bytes()).hexdigest()==entry['sha256']
    checks.update(observedFeatureCounts=counts,allPhysicalTrianglesTextured=True,
        allPhysicalTrianglesHaveAreaUVs=True,minimumUVTriangleArea=minimum_uv,
        texturedDownwardTriangles=downward,socketNodes=list(sockets.values()),
        clearOpenings=openings,sourceReferencesUnchanged=True)
    (folder/'surface-verification.json').write_text(json.dumps(checks,indent=2)+'\n')
    print(json.dumps({'ship':slug,'triangles':checks['triangles'],'closedComponents':checks['closedComponents'],
        'features':counts,'clearOpenings':len(openings)}),flush=True)
    if not do_render:return
    preview=folder/'previews'
    preview.mkdir(exist_ok=True)
    for name,camera,up in VIEWS:
        print(f'Rendering {slug}: {name}',flush=True)
        render(model,camera,up,width=1000,height=850).save(preview/(name+'.png'))
    sheet=Image.new('RGB',(1800,1260),(12,16,23))
    draw=ImageDraw.Draw(sheet)
    font=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf',22)
    titlefont=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial Bold.ttf',26)
    draw.text((24,20),f"{info['name'].upper()}  /  {info['owner']}  /  {checks['triangles']:,} triangles",font=titlefont,fill=(221,225,231))
    for i,(name,_,_) in enumerate(VIEWS[:6]):
        x,y=(i%3)*600,70+(i//3)*590
        im=Image.open(preview/(name+'.png'))
        im.thumbnail((580,520))
        sheet.paste(im,(x+(600-im.width)//2,y+8))
        draw.text((x+24,y+535),name.upper(),font=font,fill=(180,196,211))
    sheet.save(preview/'contact-sheet.png')


def gallery():
    sheet=Image.new('RGB',(1800,720),(12,16,23))
    draw=ImageDraw.Draw(sheet)
    title=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial Bold.ttf',26)
    label=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf',20)
    for i,slug in enumerate(SHIPS):
        folder=ROOT.parent/(slug+'-textured-v1')
        info=json.loads((folder/'model-info.json').read_text())
        im=Image.open(folder/'previews/three-quarter.png')
        im.thumbnail((590,580))
        sheet.paste(im,(i*600+(600-im.width)//2,60))
        draw.text((i*600+32,26),info['name'].upper(),font=title,fill=(228,229,224))
        draw.text((i*600+32,615),info['owner'],font=label,fill=(178,196,209))
        draw.text((i*600+32,649),f"{info['triangles']:,} triangles · textured GLB",font=label,fill=(130,153,174))
    sheet.save(ROOT/'trader-fleet-models.png')


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('ships',nargs='*')
    parser.add_argument('--validate-only',action='store_true')
    args=parser.parse_args()
    ships=args.ships or SHIPS
    for slug in ships:
        if slug not in SHIPS:parser.error('Unknown ship: '+slug)
        inspect(slug,not args.validate_only)
    if not args.validate_only and set(ships)==set(SHIPS):gallery()
