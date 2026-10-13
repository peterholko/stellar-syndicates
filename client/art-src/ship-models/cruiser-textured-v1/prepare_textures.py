"""Prepare a padded PBR atlas from the five-view sheet, preserving native pixels."""
from pathlib import Path
import shutil
import numpy as np
from PIL import Image

ROOT=Path(__file__).resolve().parent
OUT=ROOT/'textures'
OUT.mkdir(exist_ok=True)
source=ROOT.parents[1]/'cruiser-reference-v1/cruiser-reference.png'
if not source.exists():source=OUT/'reference-sheet.png'
if source.resolve()!=(OUT/'reference-sheet.png').resolve():
    shutil.copyfile(source,OUT/'reference-sheet.png')
rgb=np.asarray(Image.open(source).convert('RGB'),dtype=np.float32)
color=np.full((2048,2048,3),(63,68,74),dtype=np.float32)
regions={
    'top':(62,49,521,806),
    'bottom':(526,45,929,806),
    'front':(1048,110,1640,298),
    'back':(1048,355,1640,554),
    'side':(890,627,1744,798),
}
for name,(x0,y0,x1,y1) in regions.items():
    tile=rgb[y0:y1,x0:x1].copy()
    known=tile.min(axis=2)<237
    for _ in range(24):
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
    tile[~known]=(63,68,74)
    color[y0:y1,x0:x1]=tile
Image.fromarray(color.astype(np.uint8)).save(OUT/'cruiser-basecolor.png')
r,g,b=(color[:,:,i] for i in range(3))
cyan=np.clip((g-r-36)/100,0,1)*np.clip((b-r-40)/105,0,1)
emission=color*cyan[:,:,None]
Image.fromarray(np.clip(emission,0,255).astype(np.uint8)).save(OUT/'cruiser-emissive.png')
value=color.mean(axis=2)/255
rough=np.where(value>.52,.56,.48)
metal=np.where(value>.52,.38,.40)
gold=(r>b*1.3)&(r>95)&(g>b*1.12)
metal[gold]=.58;rough[gold]=.40
metal[cyan>.3]=.10
orm=np.stack([np.ones_like(r),rough,metal],axis=2)
Image.fromarray(np.clip(orm*255,0,255).astype(np.uint8)).save(OUT/'cruiser-metallic-roughness.png')
print('Prepared 2048px atlas with distinct top, bottom, side, front and back projections.')
