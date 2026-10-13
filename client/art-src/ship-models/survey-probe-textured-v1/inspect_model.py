"""Inspect exported Survey Probe GLB buffers and render seven views in software."""
from pathlib import Path
import sys
import json
import numpy as np
from PIL import Image, ImageDraw, ImageFont

ROOT=Path(__file__).resolve().parent
sys.path.insert(0,str(ROOT.parent/'mining-fleet-v1'))
from render_core import Model, render

model=Model(ROOT/'survey-probe.glb')
checks=model.validate()
components=set()
uv_min=1
downward=0
for primitive in model.primitives:
    mat=primitive['m']
    assert 'baseColorTexture' in mat['pbrMetallicRoughness']
    assert 'metallicRoughnessTexture' in mat['pbrMetallicRoughness']
    uv=primitive['uv'][primitive['i']]
    a,b=uv[:,1]-uv[:,0],uv[:,2]-uv[:,0]
    areas=np.abs(a[:,0]*b[:,1]-a[:,1]*b[:,0])/2
    assert areas.min()>1e-12
    uv_min=min(uv_min,float(areas.min()))
    downward+=int(np.count_nonzero(primitive['n'][primitive['i']].mean(axis=1)[:,1]<-.55))
    components.update(c['name'] for c in primitive['components'])
counts={
    'sensorDishes':sum(n=='sensor.forward-dish' for n in components),
    'antennaBooms':sum(n.startswith('antenna.') and n.endswith('.boom') for n in components),
    'radiatorFins':sum(n.startswith('radiator.') and n.endswith('.folded-panel') for n in components),
    'mainEngines':sum(n=='engine.main.housing' for n in components),
    'guns':sum(n.startswith(('weapon.','turret.','gun.')) for n in components),
}
assert counts==dict(sensorDishes=1,antennaBooms=4,radiatorFins=2,mainEngines=1,guns=0),counts
sockets=[n for n in model.doc['nodes'] if n['name'].startswith('socket.')]
assert len(sockets)==1 and sockets[0]['name']=='socket.engine.main'
assert sockets[0]['extras']['direction']==[0.,0.,1.]
assert 1500<=checks['triangles']<=3000
assert len(model.primitives)==2
checks.update(observedFeatureCounts=counts,allPhysicalTrianglesTextured=True,
              allPhysicalTrianglesHaveAreaUVs=True,minimumUVTriangleArea=uv_min,
              texturedDownwardTriangles=downward,socketNodes=sockets)
(ROOT/'surface-verification.json').write_text(json.dumps(checks,indent=2)+'\n')
preview=ROOT/'previews';preview.mkdir(exist_ok=True)
views=[('three-quarter',(1.2,.95,-1.45),(0,1,0)),
       ('underside',(1,-1,-1.3),(0,1,0)),
       ('top',(0,1,0),(0,0,-1)),('bottom',(0,-1,0),(0,0,-1)),
       ('front',(0,0,-1),(0,1,0)),('back',(0,0,1),(0,1,0)),
       ('side',(1,0,0),(0,1,0))]
for name,camera,up in views:
    print('Rendering',name,flush=True)
    render(model,camera,up,width=1000,height=850).save(preview/(name+'.png'))
sheet=Image.new('RGB',(1800,1200),(12,16,23));draw=ImageDraw.Draw(sheet)
try: font=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf',22)
except OSError: font=ImageFont.load_default()
for i,(name,_,_) in enumerate(views[:6]):
    x=(i%3)*600;y=(i//3)*600
    im=Image.open(preview/(name+'.png'));im.thumbnail((580,530))
    sheet.paste(im,(x+(600-im.width)//2,y+12))
    draw.text((x+24,y+555),name.upper(),font=font,fill=(190,202,213))
sheet.save(preview/'contact-sheet.png')
print(json.dumps({k:checks[k] for k in ('triangles','closedComponents','texturedDownwardTriangles','observedFeatureCounts')},indent=2))
