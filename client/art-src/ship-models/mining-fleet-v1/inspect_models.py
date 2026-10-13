"""Inspect exported assets, independently of construction; no browser required."""
from pathlib import Path
import json
import sys
import numpy as np
from PIL import Image, ImageDraw, ImageFont
from render_core import Model, render

ROOT=Path(__file__).resolve().parent


def inspect(stem):
    folder=ROOT.parent/(stem+'-textured-v1')
    model=Model(folder/(stem+'.glb'))
    checks=model.validate()
    all_uv_areas=[];physical=0;bottom=0;components=set()
    for prim in model.primitives:
        assert 'baseColorTexture' in prim['m']['pbrMetallicRoughness'],prim['name']
        assert 'metallicRoughnessTexture' in prim['m']['pbrMetallicRoughness']
        uv=prim['uv'][prim['i']]
        a,b=uv[:,1]-uv[:,0],uv[:,2]-uv[:,0]
        areas=np.abs(a[:,0]*b[:,1]-a[:,1]*b[:,0])/2
        assert areas.min()>1e-12,(prim['name'],'collapsed texture triangle')
        all_uv_areas.extend(areas.tolist());physical+=len(areas)
        normals=prim['n'][prim['i']].mean(axis=1)
        bottom+=int(np.count_nonzero(normals[:,1]<-.55))
        components.update(c['name'] for c in prim['components'])
    spec=json.loads((folder/'model-spec.json').read_text())
    assert spec['triangleBudget'][0]<=physical<=spec['triangleBudget'][1]
    nodes={n['name']:n for n in model.doc['nodes']}
    if stem=='mining-barge':
        assert len([n for n in components if n.startswith('cutter.') and n.endswith('.barrel')])==2
        assert len([n for n in components if n.startswith('engine.') and n.endswith('.housing')])==2
        for side in ('port','starboard'):
            assert nodes['socket.mining.'+side]['extras']['direction']==[0.,0.,-1.]
        observed={'cutters':2,'engines':2,'coveredHoppers':int('hopper.covered-single-bay' in components)}
    else:
        assert len([n for n in components if n.endswith('.pressure-vessel')])==6
        assert len([n for n in components if n.endswith('.keeper-pod')])==4
        assert not any(n.startswith('engine.') for n in components)
        # In top projection, the center of each berth must have no triangle.
        openings=[]
        for side in ('port','starboard'):
            xyz=np.array(nodes['socket.docking.barge.'+side]['translation'])
            q=xyz[[0,2]]
            occupied=False
            for prim in model.primitives:
                tri=prim['p'][prim['i']][:,:,[0,2]]
                edges=np.roll(tri,-1,axis=1)-tri
                rel=q-tri
                signed=edges[:,:,0]*rel[:,:,1]-edges[:,:,1]*rel[:,:,0]
                edge_a,edge_b=tri[:,1]-tri[:,0],tri[:,2]-tri[:,0]
                area=edge_a[:,0]*edge_b[:,1]-edge_a[:,1]*edge_b[:,0]
                occupied|=bool(np.any(((signed>1e-7).all(axis=1)|(signed< -1e-7).all(axis=1))&(np.abs(area)>1e-8)))
            assert not occupied,side+' berth is filled'
            openings.append(side)
        observed={'oreSilos':6,'anchoringBooms':4,'openBargeBerths':openings,'mainEngines':0}
    checks.update(texturedPhysicalTriangles=physical,texturedDownwardTriangles=bottom,
                  allPhysicalTrianglesHaveAreaUVs=True,minimumUVTriangleArea=min(all_uv_areas),
                  observedFeatureCounts=observed,socketNodes=[n for n in nodes if n.startswith('socket.')])
    (folder/'surface-verification.json').write_text(json.dumps(checks,indent=2)+'\n')
    preview=folder/'previews';preview.mkdir(exist_ok=True)
    views=[('three-quarter',(1.2,.95,-1.45),(0,1,0)),('underside',(1,-1,-1.3),(0,1,0)),
           ('top',(0,1,0),(0,0,-1)),('bottom',(0,-1,0),(0,0,-1)),
           ('front',(0,0,-1),(0,1,0)),('back',(0,0,1),(0,1,0)),('side',(1,0,0),(0,1,0))]
    for name,camera,up in views:
        print(stem,name,flush=True)
        render(model,camera,up,width=1100,height=850).save(preview/(name+'.png'))
    sheet=Image.new('RGB',(1650,1314),(12,16,23));draw=ImageDraw.Draw(sheet)
    try: font=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf',22)
    except OSError: font=ImageFont.load_default()
    for i,(name,_,_) in enumerate(views[:6]):
        x=(i%3)*550;y=(i//3)*657
        im=Image.open(preview/(name+'.png'));im.thumbnail((550,595))
        sheet.paste(im,(x+(550-im.width)//2,y+25))
        draw.text((x+24,y+603),name.upper(),font=font,fill=(190,202,213))
    sheet.save(preview/'contact-sheet.png')
    print(json.dumps({k:checks[k] for k in ('triangles','closedComponents','texturedPhysicalTriangles','observedFeatureCounts')},indent=2))


if __name__=='__main__':
    for stem in (sys.argv[1:] or ['mining-barge','mining-belt-station']): inspect(stem)
