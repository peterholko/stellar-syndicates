"""Verify and render the actual Combat Freighter GLB, independently of authoring."""
from pathlib import Path
import argparse
import hashlib
import json
import sys

import numpy as np
from PIL import Image,ImageDraw,ImageFont

ROOT=Path(__file__).resolve().parent
sys.path.insert(0,str(ROOT.parent/'mining-fleet-v1'))
sys.path.insert(0,str(ROOT.parent/'trader-fleet-v1'))
from render_core import Model,render
from audit_textures import collect,unlit

VIEWS=[('three-quarter',(1.2,.95,-1.45),(0,1,0)),('port-quarter',(-1.2,.8,-1.45),(0,1,0)),
       ('underside',(1,-1,-1.3),(0,1,0)),('top',(0,1,0),(0,0,-1)),
       ('bottom',(0,-1,0),(0,0,-1)),('front',(0,0,-1),(0,1,0)),
       ('back',(0,0,1),(0,1,0)),('side',(-1,0,0),(0,1,0))]


def verify(render_views=True):
    path=ROOT/'combat-freighter.glb';model=Model(path)
    checks=model.validate()
    components={c['name'] for p in model.primitives for c in p['components']}
    counts=dict(cargoCitadels=int('cargo.citadel' in components),
        holdDoors=sum(n.startswith('cargo.hold-door.') for n in components),
        grappleArms=sum(n.startswith('grapple.') and n.endswith('.palm') for n in components),
        grappleFingers=sum(n.startswith('grapple.') and '.finger.' in n and n.endswith('.tip') for n in components),
        mainEngines=sum(n.startswith('engine.') and n.endswith('.housing') for n in components),
        guns=sum(n.startswith(('weapon.','gun.','turret.')) for n in components))
    assert counts==dict(cargoCitadels=1,holdDoors=2,grappleArms=4,grappleFingers=12,mainEngines=3,guns=0),counts
    assert 5000<=checks['triangles']<=6000,checks['triangles']
    sockets={n['name']:n for n in model.doc['nodes'] if n['name'].startswith('socket.')}
    assert sorted(sockets)==['socket.engine.centre','socket.engine.port','socket.engine.starboard']
    for name,node in sockets.items():
        assert np.allclose(node['extras']['direction'],[0,0,1]),name
        assert abs(node['translation'][1])<1e-8,(name,'engine axes must share one height')
    assert len(model.primitives)==2 and len(model.images)==3
    assert all(im.shape==(2048,2048,3) for im in model.images)
    measurements=collect(model)
    for row in measurements:
        assert row['areaWithAnisotropyOver4Pct']==0,row
        assert row['areaBelow8TexelsPerUnitPct']==0,row
    color=model.images[0];emission=model.images[2]
    amber=(color[:,:,0]>color[:,:,1]*1.1)&(color[:,:,1]>color[:,:,2]*1.7)&(color[:,:,0]>.4)
    assert emission[amber].max()<1e-5,'Amber stripe must not emit light'
    bounds=np.ptp(model.positions,axis=0)
    assert .48<bounds[0]/bounds[2]<.59,bounds
    checks.update(modelSHA256=hashlib.sha256(path.read_bytes()).hexdigest(),featureCounts=counts,
        engineSockets=list(sockets),allEngineAxesSameHeight=True,weaponSockets=0,
        widthToLength=float(bounds[0]/bounds[2]),embeddedPBRMaps=3,amberIsNonEmissive=True,
        maximumUVAnisotropy=max(x['maximumAnisotropy'] for x in measurements),
        minimumTexelsPerUnit=min(x['minimumTexelsPerUnit'] for x in measurements))
    (ROOT/'surface-verification.json').write_text(json.dumps(checks,indent=2)+'\n')
    (ROOT/'verification/texture-audit.json').write_text(json.dumps({'modelSHA256':checks['modelSHA256'],'components':measurements},indent=2)+'\n')
    print(json.dumps({k:checks[k] for k in ['triangles','closedComponents','featureCounts','widthToLength','maximumUVAnisotropy','minimumTexelsPerUnit']}))
    if render_views:
        out=ROOT/'previews';out.mkdir(exist_ok=True)
        sheet=Image.new('RGB',(1800,1410),(12,16,22));draw=ImageDraw.Draw(sheet)
        font=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf',20)
        for i,(name,camera,up) in enumerate(VIEWS):
            print('Rendering',name,flush=True)
            im=render(model,camera,up=up,width=1050,height=800);im.save(out/f'{name}.png')
            x=(i%3)*600;y=(i//3)*470
            sheet.paste(im.resize((600,438)),(x,y+32));draw.text((x+15,y+8),name,font=font,fill='white')
        sheet.save(out/'contact-sheet.png')
        for name,camera,up in [VIEWS[0],VIEWS[2],VIEWS[6]]:
            render(unlit(model),camera,up=up,width=1050,height=800).save(out/f'{name}-unlit.png')


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--validate-only',action='store_true')
    verify(not parser.parse_args().validate_only)
