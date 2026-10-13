"""Compose actual renderer captures for visual QA and the asset gallery."""
from PIL import Image,ImageDraw,ImageFont,ImageChops
from scaffold import SPECS,ROOT,MODELS
import numpy as np

font=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf',24)
def tile(ship,view,w=740,h=480):
    im=Image.open(MODELS/(ship+'-textured-v1')/'previews'/(view+'.png')).convert('RGB')
    pixels=np.asarray(im).astype('int16');mask=np.max(np.abs(pixels-pixels[0,0]),axis=2)>18
    y,x=np.nonzero(mask);box=(max(0,x.min()-30),max(0,y.min()-30),min(im.width,x.max()+31),min(im.height,y.max()+31))
    crop=im.crop(box);crop.thumbnail((w-40,h-75),Image.Resampling.LANCZOS)
    out=Image.new('RGB',(w,h),(13,20,30));out.paste(crop,((w-crop.width)//2,45+(h-60-crop.height)//2))
    ImageDraw.Draw(out).text((22,15),SPECS[ship]['title']+' · '+view,fill=(210,229,239),font=font)
    return out

ids=list(SPECS)
for batch in range(3):
    board=Image.new('RGB',(1480,1440),(13,20,30))
    for row,ship in enumerate(ids[batch*3:batch*3+3]):
        for col,view in enumerate(('perspective','bottom-quarter')):board.paste(tile(ship,view),(col*740,row*480))
    board.save(ROOT/f'qa-models-{batch+1}.jpg',quality=93)
for view in ('top','side','back'):
    board=Image.new('RGB',(1800,1200),(13,20,30))
    for i,ship in enumerate(ids):board.paste(tile(ship,view,600,400),((i%3)*600,(i//3)*400))
    board.save(ROOT/f'qa-{view}.jpg',quality=94)
board=Image.new('RGB',(1800,1350),(13,20,30))
for i,ship in enumerate(ids):board.paste(tile(ship,'front-quarter',600,450),((i%3)*600,(i//3)*450))
board.save(ROOT/'nine-new-ships.jpg',quality=95)
print('Saved renderer contact sheets.')
