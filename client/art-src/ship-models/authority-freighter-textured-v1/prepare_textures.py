"""Pack the approved five-view artwork for projection onto the modeled surfaces.

Source pixels are retained at native resolution. Only non-surface white margins
are dilated to prevent white fringes at UV seams and mipmap boundaries.
"""
from pathlib import Path
import shutil
import numpy as np
from PIL import Image

ROOT=Path(__file__).resolve().parent
OUT=ROOT/'textures'
OUT.mkdir(exist_ok=True)
source=ROOT.parents[1]/'authority-freighter-reference-v1/authority-freighter-reference.png'
if not source.exists():source=OUT/'reference-sheet.png'
if source.resolve()!=(OUT/'reference-sheet.png').resolve():
    shutil.copyfile(source,OUT/'reference-sheet.png')
rgb=np.asarray(Image.open(source).convert('RGB'),dtype=np.float32)
color=np.full((2048,2048,3),(62,67,74),dtype=np.float32)
import json
regions=json.loads((ROOT/'model-spec.json').read_text())['regions']
for name,(x0,y0,x1,y1) in regions.items():
    tile=rgb[y0:y1,x0:x1].copy()
    known=tile.min(axis=2)<237
    for _ in range(36):
        padded=np.pad(tile*known[:,:,None],((1,1),(1,1),(0,0)))
        valid=np.pad(known.astype(np.float32),1)
        total=np.zeros_like(tile);count=np.zeros_like(known,dtype=np.float32)
        h,w=known.shape
        for dy,dx in [(0,1),(1,0),(1,2),(2,1),(0,0),(0,2),(2,0),(2,2)]:
            total+=padded[dy:dy+h,dx:dx+w]
            count+=valid[dy:dy+h,dx:dx+w]
        fill=(~known)&(count>0)
        tile[fill]=total[fill]/count[fill,None]
        known|=fill
    tile[~known]=(62,67,74)
    color[y0:y1,x0:x1]=tile
Image.fromarray(color.astype(np.uint8)).save(OUT/'authority-freighter-basecolor.png')
r,g,b=(color[:,:,i] for i in range(3))
cyan=np.clip((g-r-34)/100,0,1)*np.clip((b-r-37)/100,0,1)
Image.fromarray(np.clip(color*cyan[:,:,None],0,255).astype(np.uint8)).save(OUT/'authority-freighter-emissive.png')
value=color.mean(axis=2)/255
rough=np.where(value>.52,.58,.48)
metal=np.where(value>.52,.32,.40)
gold=(r>b*1.3)&(r>95)&(g>b*1.12)
metal[gold]=.55;rough[gold]=.40
metal[cyan>.3]=.08;rough[cyan>.3]=.26
orm=np.stack([np.ones_like(r),rough,metal],axis=2)
Image.fromarray(np.clip(orm*255,0,255).astype(np.uint8)).save(OUT/'authority-freighter-metallic-roughness.png')
print('Packed native reference artwork into three 2048px PBR maps, with a dedicated underside projection.')
