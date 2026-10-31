"""Close-up visual inspection of the exported Second Wind cabin, before/after."""
from pathlib import Path
import argparse
import copy
import hashlib
import json
import sys

import numpy as np
from PIL import Image, ImageDraw

ROOT=Path(__file__).resolve().parent
sys.path.insert(0,str(ROOT.parent/'mining-fleet-v1'))
from render_core import Model,render

VIEWS=[('port-forward',(-1.3,.75,-1.5)),('port-aft',(-1.3,.75,1.5)),
       ('starboard-aft',(1.3,.75,1.5)),('top',(0,1,0))]


def window_alignment(model):
    """Sample real GLB surfaces under the reference's window interiors.

    A complete windscreen must occupy one plane; a low stretch ratio alone
    misses a roof ridge cutting straight through the painted window.
    """
    triangles=[]
    for primitive in model.primitives:
        for component in primitive['components']:
            if component['name']=='bridge.glazing':
                start=component['indexStart']//3
                indices=primitive['i'][start:start+component['indexCount']//3]
                triangles.extend(zip(primitive['p'][indices],primitive['uv'][indices]))
    def sample(pixel):
        q=np.array([(pixel[0]-278)/38,(pixel[1]-437)/38])
        hits=[]
        for positions,uv in triangles:
            projected=positions[:,[0,2]].astype(float)
            matrix=np.column_stack([projected[1]-projected[0],projected[2]-projected[0]])
            if abs(np.linalg.det(matrix))<1e-10:continue
            b,c=np.linalg.solve(matrix,q-projected[0]);bary=np.array([1-b-c,b,c])
            if bary.min()>=-1e-6:
                hits.append((float(bary@positions[:,1]),bary@uv*2048))
        height,texel=max(hits,key=lambda x:x[0])
        return np.array([q[0],height,q[1]]),float(np.linalg.norm(texel-pixel))
    samples=[sample(pixel) for pixel in [(122,292),(158,292),(120,319),(170,319)]]
    points=np.array([x[0] for x in samples])
    _,_,axes=np.linalg.svd(points-points.mean(0))
    error=float(np.abs((points-points.mean(0))@axes[-1]).max())
    roof=[sample(pixel)[0][1] for pixel in [(111,250),(155,250),(120,270),(150,270)]]
    return dict(windscreenPlaneError=round(error,7),
        roofHeightRange=round(float(np.ptp(roof)),7),
        maximumSourcePixelError=round(max(x[1] for x in samples),5),
        windscreenSampleHeights=[round(float(x),5) for x in points[:,1]])


def non_bridge_geometry(model):
    result={}
    for p in model.primitives:
        for c in p['components']:
            if c['name'].startswith('bridge.'):continue
            start=c['indexStart']//3
            i=p['i'][start:start+c['indexCount']//3]
            result.setdefault(c['name'],[]).append(np.concatenate([p['p'][i],p['n'][i],p['uv'][i]],axis=2).tobytes())
    return result


def inspect(phase,measure_only=False):
    output=ROOT/'command-module-repair'/phase
    output.mkdir(exist_ok=True,parents=True)
    path=(ROOT/'command-module-repair/second-wind-before.glb' if phase=='before'
          else ROOT.parent/'second-wind-textured-v1/second-wind.glb')
    model=Model(path)
    model.validate()
    cabin=copy.copy(model)
    cabin.primitives=[]
    for primitive in model.primitives:
        parts=[]
        for component in primitive['components']:
            if component['name'].startswith('bridge.'):
                start=component['indexStart']//3
                parts.append(primitive['i'][start:start+component['indexCount']//3])
        if parts:
            p=primitive.copy()
            p['i']=np.concatenate(parts)
            p['m']=copy.deepcopy(p['m'])
            p['m'].setdefault('extensions',{})['KHR_materials_unlit']={}
            cabin.primitives.append(p)
    cabin.positions=np.concatenate([p['p'][p['i'].reshape(-1)] for p in cabin.primitives])
    sheet=Image.new('RGB',(1600,1280),(12,16,22))
    draw=ImageDraw.Draw(sheet)
    for i,(name,view) in enumerate([] if measure_only else VIEWS):
        im=render(cabin,view,up=(0,0,-1) if name=='top' else (0,1,0),width=1000,height=760)
        im.save(output/f'{name}.png')
        x=(i%2)*800;y=(i//2)*640
        sheet.paste(im.resize((800,608)),(x,y+32))
        draw.text((x+20,y+10),f'{phase.upper()} / {name}',fill='white')
    if not measure_only:sheet.save(output/'contact-sheet.png')
    alignment=window_alignment(model)
    unchanged=True
    if phase=='after':
        assert alignment['windscreenPlaneError']<1e-5,alignment
        assert alignment['roofHeightRange']<1e-5,alignment
        assert alignment['maximumSourcePixelError']<.001,alignment
        before=Model(ROOT/'command-module-repair/second-wind-before.glb')
        unchanged=non_bridge_geometry(before)==non_bridge_geometry(model)
        assert unchanged,'Unrelated geometry or UVs changed'
    (output/'inspection.json').write_text(json.dumps({'modelSHA256':hashlib.sha256(path.read_bytes()).hexdigest(),
        'method':'Actual exported bridge triangles and embedded texture, unlit, four angles',
        'views':[x[0] for x in VIEWS], 'windowAlignment':alignment,
        'nonBridgeGeometryAndUVsUnchanged':unchanged if phase=='after' else None},indent=2)+'\n')
    print(json.dumps({'phase':phase,'windowAlignment':alignment,'nonBridgeGeometryAndUVsUnchanged':unchanged}))


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('phase',choices=['before','after'])
    parser.add_argument('--measure-only',action='store_true')
    args=parser.parse_args()
    inspect(args.phase,args.measure_only)
