"""Verify the exported material variant and inspect dark-background readability.

These are software inspection renders, not screenshots of the running game.
The background uses the actual galaxy map's #05070d base color. The model has
no bloom, selection outline or silhouette light added for these renders.
"""
from pathlib import Path
import hashlib
import json
import numpy as np
from PIL import Image,ImageDraw,ImageFont
from render_core import Model,render

ROOT=Path(__file__).resolve().parent
OUT=ROOT/'previews'


def galaxy_background(width,height):
    canvas=Image.new('RGB',(width,height),'#05070d')
    draw=ImageDraw.Draw(canvas)
    rng=np.random.default_rng(6173)
    points=rng.uniform([0,0],[width,height],(150,2))
    for i in range(0,144,7):
        a,b=points[i],points[i+3]
        if np.linalg.norm(a-b)<width*.21:
            draw.line([tuple(a),tuple(b)],fill=(15,23,36),width=1)
    for x,y in points:
        tone=int(rng.uniform(25,83))
        draw.point((x,y),fill=(tone,tone+6,tone+14))
    return np.asarray(canvas,dtype=np.float32)/255


def inspect():
    ivory=Model(ROOT/'authority-arbiter.glb')
    dark=Model(ROOT/'authority-arbiter-dark.glb')
    checks=dark.validate()
    for before,after in zip(ivory.primitives,dark.primitives,strict=True):
        for key in ('p','n','uv','i'):assert np.array_equal(before[key],after[key]),key
        assert before['components']==after['components']
        assert np.isfinite(after['color']).all() and after['color'].min()>=0 and after['color'].max()<=1
    assert len(ivory.node_world)==len(dark.node_world)
    for name,matrix in ivory.node_world.items():
        if name=='Authority Arbiter':continue
        assert np.array_equal(matrix,dark.node_world[name]),name
    for before,after in zip(ivory.doc['images'],dark.doc['images'],strict=True):
        assert ivory.view(before['bufferView'])==dark.view(after['bufferView'])
    for before,after in zip(ivory.doc['materials'],dark.doc['materials'],strict=True):
        assert before['emissiveFactor']==after['emissiveFactor']
    checks.update(geometryAndUVsExactlyMatchIvory=True,embeddedTextureBytesExactlyMatchIvory=True,
        allSocketAndAssemblyTransformsPreserved=True,originalEmissionPreserved=True,
        standardGLTFVertexColorMaterials=True,addedTriangles=0,addedRenderSections=0,
        sourceModelSHA256=hashlib.sha256((ROOT/'authority-arbiter.glb').read_bytes()).hexdigest(),
        browserTested=False,gameIntegrationChanged=False)
    for name,camera,up in [('dark-galaxy',(1,.85,-1.4),(0,1,0)),
                           ('dark-underside',(1,-.9,-1.3),(0,1,0)),
                           ('dark-rear',(-1,.80,1.3),(0,1,0))]:
        print('Rendering '+name,flush=True)
        render(dark,camera,up,width=1500,height=1100,background=galaxy_background(1500,1100)).save(OUT/(name+'.png'))

    # Small-scale silhouette checks include the thin halo and underside. These
    # contrast statistics describe only this reproducible inspection setup.
    checks['readabilitySamples']=[]
    tiles=[]
    for title,camera,up,size,light in [
        ('TOP · 256 PX',(0,1,0),(0,0,-1),256,1),
        ('TOP · 128 PX',(0,1,0),(0,0,-1),128,1),
        ('TOP · 40% LIGHT',(0,1,0),(0,0,-1),256,.4),
        ('UNDERSIDE · 40% LIGHT',(0,-1,0),(0,0,-1),256,.4),
    ]:
        print('Inspecting '+title,flush=True)
        background=np.array([5,7,13],np.float32)/255
        result,mask=render(dark,camera,up,width=size,height=size,padding=12,
            background=background,light_scale=light,return_mask=True)
        rgb=np.asarray(result,dtype=float)/255
        luma=(rgb-background)@np.array([.2126,.7152,.0722])
        interior=mask.copy()
        for axis in (0,1):
            for sign in (-1,1):interior&=np.roll(mask,sign,axis)
        edge=mask&~interior
        visible=float(np.mean(luma[edge]>12/255))
        checks['readabilitySamples'].append(dict(view=title,canvasPixels=size,lightScale=light,
            edgePixelsAboveBackgroundBy12Of255=round(visible,4),
            medianShipLumaAboveBackground=round(float(np.median(luma[mask])),4)))
        # Do not let an almost invisible outline pass merely because its tiny
        # windows are bright. Most physical silhouette pixels must separate.
        assert visible>.80,(title,visible)
        tiles.append((title,result))

    print('Rendering reduced-light hero',flush=True)
    render(dark,(1,.85,-1.4),width=1500,height=1100,background=galaxy_background(1500,1100),light_scale=.4).save(OUT/'dark-low-light.png')
    try:
        font=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf',20)
        small=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf',16)
    except OSError:font=small=ImageFont.load_default()
    strip=Image.new('RGB',(1280,365),'#05070d');draw=ImageDraw.Draw(strip)
    for i,(title,tile) in enumerate(tiles):
        x=i*320+(320-tile.width)//2
        strip.paste(tile,(x,35+(256-tile.height)//2))
        draw.text((i*320+160,307),title,fill='#a9bbc9',font=small,anchor='mm')
    draw.text((640,349),'Actual exported model · no bloom or outline · near-black galaxy background',font=small,anchor='mm',fill='#65778b')
    strip.save(OUT/'dark-map-readability.png')

    print('Rendering ivory/dark comparison',flush=True)
    comparison=Image.new('RGB',(1800,740),'#05070d');draw=ImageDraw.Draw(comparison)
    for i,(label,model) in enumerate([('IVORY ARMOR',ivory),('DARK IMPERIAL ARMOR',dark)]):
        tile=render(model,(1,.85,-1.4),width=900,height=660,padding=45,background=galaxy_background(900,660))
        comparison.paste(tile,(i*900,0))
        draw.text((i*900+450,685),label,font=font,anchor='mm',fill='#c4c5c4')
    draw.text((900,721),'Same geometry, camera and illumination',font=small,anchor='mm',fill='#788a9c')
    comparison.save(OUT/'ivory-dark-comparison.png')
    checks['renderingMethod']='Actual exported triangles, embedded textures and linear vertex colors; software lighting, not WebGL. Background base #05070d matches client/src/render.ts.'
    (ROOT/'dark-surface-verification.json').write_text(json.dumps(checks,indent=2)+'\n')
    print(json.dumps({k:v for k,v in checks.items() if k not in ('positiveVolumes',)},indent=2),flush=True)


if __name__=='__main__':inspect()
