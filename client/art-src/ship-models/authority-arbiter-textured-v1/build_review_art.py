"""Top-view review board and transparent candidate sprites from the exported GLB.

Software inspection lighting is used because browser rendering is unavailable.
These are staged art-review assets; the live game's sprite catalog is untouched.
"""
from pathlib import Path
import hashlib,json
import numpy as np
from PIL import Image,ImageDraw,ImageFont
from render_core import Model,render

ROOT=Path(__file__).resolve().parent


def top(model,size=1536):
    rgb,mask=render(model,(0,1,0),(0,0,-1),width=size,height=size,
        padding=size*(1-2/2.24),background=np.zeros(3),return_mask=True)
    result=rgb.convert('RGBA');result.putalpha(Image.fromarray(mask.astype('uint8')*255))
    return result


def fit(source,height):
    cropped=source.crop(source.getbbox())
    return cropped.resize((round(cropped.width*height/cropped.height),height),Image.Resampling.LANCZOS)


def build():
    sprites=ROOT/'sprites';sprites.mkdir(exist_ok=True)
    model=Model(ROOT/'authority-arbiter-dark.glb')
    print('Rendering transparent dark top sprite',flush=True)
    current=top(model)
    files=[]
    for size in (1024,512,256,128):
        path=sprites/f'authority-arbiter-dark-top-{size}.png'
        output=current.resize((size,size),Image.Resampling.LANCZOS)
        output.save(path,optimize=True)
        alpha=np.asarray(output.getchannel('A'))
        assert alpha.max()==255 and alpha.min()==0
        files.append(dict(file=str(path.relative_to(ROOT)),size=[size,size],bytes=path.stat().st_size,
                          sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
    approved=Image.open(ROOT/'concept.png').convert('RGBA')
    previous_path=ROOT/'revisions/long-mass-driver-v1/authority-arbiter-dark.glb'
    print('Rendering prior model for the design comparison',flush=True)
    previous=top(Model(previous_path))
    board=Image.new('RGB',(1800,1230),'#05070d');draw=ImageDraw.Draw(board)
    try:
        title=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf',23)
        small=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf',17)
    except OSError:title=small=ImageFont.load_default()
    for i,(label,source) in enumerate([('APPROVED SPRITE',approved),('PREVIOUS MODEL',previous),('REVISED DARK MODEL',current)]):
        center=i*600+300
        draw.text((center,38),label,font=title,anchor='mm',fill='#d6c99f')
        display=fit(source,850)
        board.paste(display,(center-display.width//2,83),display)
        thumb=Image.new('RGBA',(128,128))
        hull=fit(source,114)
        thumb.alpha_composite(hull,((128-hull.width)//2,7))
        board.paste(thumb,(center-64,1000),thumb)
        draw.text((center,1151),'128 PX',font=small,anchor='mm',fill='#889baa')
    draw.text((900,1202),'Same overall display length · direct overhead view · dark galaxy background',font=small,anchor='mm',fill='#8593a4')
    board.save(ROOT/'previews/top-design-review.png',optimize=True)
    info=json.loads((ROOT/'dark-model-info.json').read_text())
    manifest=dict(asset='Authority Arbiter',variant='dark-imperial',stage='art-review, not live game catalog',
        sourceModel='authority-arbiter-dark.glb',sourceSHA256=hashlib.sha256((ROOT/'authority-arbiter-dark.glb').read_bytes()).hexdigest(),
        projection='Orthographic top, nose up',frameLengthToHullLength=2.24/2,
        triangles=info['triangles'],rendering='Actual GLB geometry, materials and textures; software inspection lighting, not a WebGL/PBR bake',
        outputs=files)
    (ROOT/'review-sprite-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print(json.dumps(manifest,indent=2),flush=True)


if __name__=='__main__':build()
