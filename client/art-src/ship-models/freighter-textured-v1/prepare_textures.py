"""Prepare native reference colors and derived PBR maps; no repainting or resizing."""
from pathlib import Path
import shutil
import numpy as np
from PIL import Image

ROOT=Path(__file__).resolve().parent
OUT=ROOT/'textures'
OUT.mkdir(exist_ok=True)
source=ROOT.parents[1]/'ships-2026-09-08/freighter.png'
if not source.exists():source=OUT/'reference-top.png'
if source.resolve()!=(OUT/'reference-top.png').resolve():
    shutil.copyfile(source,OUT/'reference-top.png')
iso=ROOT.parents[1]/'ships-isometric-v1/freighter.png'
if iso.exists():shutil.copyfile(iso,OUT/'reference-isometric.png')
rgba=np.asarray(Image.open(source).convert('RGBA'),dtype=np.float32)
tile=rgba[:,:,:3].copy()
known=rgba[:,:,3]>128
for _ in range(24):
    padded=np.pad(tile*known[:,:,None],((1,1),(1,1),(0,0)))
    valid=np.pad(known.astype(np.float32),1)
    total=np.zeros_like(tile)
    count=np.zeros_like(known,dtype=np.float32)
    h,w=known.shape
    for dy,dx in [(0,1),(1,0),(1,2),(2,1),(0,0),(0,2),(2,0),(2,2)]:
        total+=padded[dy:dy+h,dx:dx+w]
        count+=valid[dy:dy+h,dx:dx+w]
    fill=(~known)&(count>0)
    tile[fill]=total[fill]/count[fill,None]
    known|=fill
tile[~known]=(49,56,62)
color=np.full((2048,2048,3),(49,56,62),dtype=np.float32)
color[:tile.shape[0],:tile.shape[1]]=tile
Image.fromarray(color.astype(np.uint8)).save(OUT/'freighter-basecolor.png')
r,g,b=(color[:,:,i] for i in range(3))
cyan=np.clip((g-r-38)/95,0,1)*np.clip((b-r-45)/95,0,1)
Image.fromarray(np.clip(color*cyan[:,:,None],0,255).astype(np.uint8)).save(OUT/'freighter-emissive.png')
value=color.mean(axis=2)/255
rough=np.where(value>.52,.55,.49)
metal=np.where(value>.52,.38,.42)
gold=(r>b*1.3)&(r>95)&(g>b*1.12)
metal[gold]=.57
rough[gold]=.43
metal[cyan>.3]=.10
orm=np.stack([np.ones_like(r),rough,metal],axis=2)
Image.fromarray(np.clip(orm*255,0,255).astype(np.uint8)).save(OUT/'freighter-metallic-roughness.png')
print('Prepared 2048px color, emission, and metallic/roughness maps from original freighter pixels.')
