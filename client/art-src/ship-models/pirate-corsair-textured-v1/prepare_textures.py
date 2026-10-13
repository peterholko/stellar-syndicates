"""Pack view pixels as a padded model UV atlas and derive material channels.

The original generated reference is preserved unchanged. This is model texture
preparation, not a revised reference drawing or an invented underside.
"""
from pathlib import Path
import json
import numpy as np
from PIL import Image

ROOT=Path(__file__).resolve().parent
SPEC=json.loads((ROOT/'model-spec.json').read_text())
OUT=ROOT/'textures';OUT.mkdir(exist_ok=True)
rgb=np.asarray(Image.open(OUT/'reference-sheet.png').convert('RGB'),dtype=np.float32)
color=np.full((2048,2048,3),(53,51,47),dtype=np.float32)
for name,(x0,y0,x1,y1) in SPEC['regions'].items():
    tile=rgb[y0:y1,x0:x1].copy()
    # Grow surface colors into white margins so bevel UVs/mipmaps cannot sample
    # the sheet's white background. View labels are outside these crop bounds.
    known=tile.min(axis=2)<223
    for _ in range(40):
        padded=np.pad(tile*known[:,:,None],((1,1),(1,1),(0,0)))
        valid=np.pad(known.astype(np.float32),1)
        total=np.zeros_like(tile);count=np.zeros_like(known,dtype=np.float32)
        h,w=known.shape
        for dy,dx in [(0,1),(1,0),(1,2),(2,1),(0,0),(0,2),(2,0),(2,2)]:
            total+=padded[dy:dy+h,dx:dx+w];count+=valid[dy:dy+h,dx:dx+w]
        fill=(~known)&(count>0)
        tile[fill]=total[fill]/count[fill,None];known|=fill
    tile[~known]=(53,51,47)
    color[y0:y1,x0:x1]=tile
ship=SPEC['id']
Image.fromarray(color.astype(np.uint8)).save(OUT/f'{ship}-basecolor.png')
r,g,b=(color[:,:,i] for i in range(3))
hot=np.clip((r-215)/40,0,1)*np.clip((g-105)/70,0,1)*np.clip((g-b-35)/100,0,1)
Image.fromarray(np.clip(color*hot[:,:,None],0,255).astype(np.uint8)).save(OUT/f'{ship}-emissive.png')
paint=(r>g*1.15)&(g>b*1.1)&(r>90)
rough=np.where(paint,.78,.59);metal=np.where(paint,.12,.68)
rough[hot>.2]=.28;metal[hot>.2]=.06
orm=np.stack([np.ones_like(r),rough,metal],axis=2)
Image.fromarray(np.clip(orm*255,0,255).astype(np.uint8)).save(OUT/f'{ship}-metallic-roughness.png')
print(f'{ship}: original five-view pixels → 2048px base color, emission and metallic/roughness maps.')
