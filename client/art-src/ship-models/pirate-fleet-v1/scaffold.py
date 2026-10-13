"""Reuse the project's deterministic mesh exporter; never invokes Blender."""
from pathlib import Path
import json
import shutil

ROOT = Path(__file__).resolve().parent
BASE = ROOT.parent
SHIPS = ['privateer-raider', 'pirate-corsair', 'pirate-boarding-raider']

def scaffold(ship):
    spec = json.loads((ROOT/'specs'/f'{ship}.json').read_text())
    out = BASE/f'{ship}-textured-v1'
    out.mkdir(exist_ok=True)
    for child in ['textures', 'previews', 'source']:
        (out/child).mkdir(exist_ok=True)
    (out/'model-spec.json').write_text(json.dumps(spec, indent=2)+'\n')
    reference = BASE.parent/f'{ship}-reference-v1'/f'{ship}-reference.png'
    if reference.exists():
        shutil.copyfile(reference, out/'textures/reference-sheet.png')
    template = ROOT/'templates'
    if not template.exists():
        template = BASE/'scout-textured-v1'
    core = (template/'mesh_core.py').read_text()
    core = core.replace('scout', ship).replace('SCOUT', spec['title'].upper()).replace('Scout', spec['title'])
    core = core.replace('Reference artwork | titanium, brass, panels and canopy', 'Reference artwork | salvaged steel, hazard paint and machinery')
    core = core.replace('cyan = solid("Cyan luminous apertures", (.003,.15,.30),.1,.4,3.0)',
                        'amber = solid("Amber luminous apertures", (1.0,.29,.015),.05,.3,1.0)')
    core = core.replace('brass = solid("Warm brass collars", (.52,.30,.025),.60,.40)',
                        'brass = solid("Oxidized bronze collars", (.24,.14,.055),.6,.52)')
    core = core.replace('graphite = solid("Structural frames | gunmetal", (.025,.030,.037),.42,.55)',
                        'graphite = solid("Structural frames | gunmetal", (.075,.08,.085),.48,.56)')
    # Register independent center lines: generated mirrored views can have
    # slightly different extents, especially the asymmetric Privateer wings.
    a = core.index('def bottom_uv(v):')
    b = core.index('\ndef side_uv(v):', a)
    core = core[:a]+'''def bottom_uv(v):
    tx0,ty0,tx1,ty1=SPEC['top'];bx0,by0,bx1,by1=SPEC['bottom']
    bottom_center=SPEC['bottomCenter']
    dx=v[0]*S
    factor=(bottom_center-bx0)/max(1,tx1-CX) if dx>=0 else (bx1-bottom_center)/max(1,CX-tx0)
    return ((bottom_center-dx*factor)/2048,
            (by0+(CY-v[1]*S-ty0)/(ty1-ty0)*(by1-by0))/2048)
''' + core[b:]
    # Prevent projections along steep shoulders from stretching top pixels down
    # vertical walls. Use the dominant face direction, then keep side/front UVs
    # inside the illustrated silhouette (the AI views vary slightly in height).
    core = core.replace('import numpy as np', 'import numpy as np\nfrom PIL import Image')
    start = core.index('def side_uv(v):')
    finish = core.index('\ndef back_uv(v):', start)
    core = core[:start]+'''_reference = np.asarray(Image.open(ROOT/'textures/reference-sheet.png').convert('RGB'))
_projection_limits = {}
for _view in ['side', 'front']:
    _x0,_y0,_x1,_y1 = SPEC[_view]
    _mask = _reference[_y0:_y1,_x0:_x1].min(axis=2)<180
    _limits = []
    for _x in range(_mask.shape[1]):
        _ys = np.flatnonzero(_mask[:,_x])
        _limits.append((_y0+_ys[0],_y0+_ys[-1]) if len(_ys) else None)
    _valid = [i for i,x in enumerate(_limits) if x is not None]
    for _x in range(len(_limits)):
        if _limits[_x] is None:
            _limits[_x] = _limits[min(_valid,key=lambda n:abs(n-_x))]
    _projection_limits[_view] = _limits

def registered_uv(view,px,py):
    x0,y0,x1,y1=SPEC[view]
    ix=max(0,min(x1-x0-1,int(round(px-x0))))
    low,high=_projection_limits[view][ix]
    inset=min(3,(high-low)*.22)
    return (max(x0,min(x1-1,px))/2048,max(low+inset,min(high-inset,py))/2048)

def side_uv(v):
    x0,y0,x1,y1=SPEC['side'];a,b,c,d=SPEC['top']
    py=CY-v[1]*S
    return registered_uv('side', x0+(py-b)/(d-b)*(x1-x0), SPEC['sideBase']-v[2]*SPEC['sideScale'])

def front_uv(v):
    x0,y0,x1,y1=SPEC['front'];a,b,c,d=SPEC['top']
    return registered_uv('front', (x0+x1)/2-v[0]*S*(x1-x0)/(c-a), SPEC['frontBase']-v[2]*SPEC['frontScale'])
''' + core[finish:]
    start = core.index('def hull_mapping(normal,center=None):')
    finish = core.index('\ndef panel_uv(', start)
    core = core[:start]+'''def hull_mapping(normal,center=None):
    axis=int(np.argmax(np.abs(normal)))
    if axis==2:return (ventral,bottom_uv) if normal[2]<0 else (atlas,top_uv)
    if axis==0:return atlas,side_uv
    return atlas,front_uv if normal[1]>0 else back_uv
''' + core[finish:]
    # Project a rear-facing wall into an exposed plating region local to that
    # component. Recessed engine mouths have their own registered rear disks.
    core = core.replace('"source":"../../'+ship+'-reference-v1/'+ship+'-reference.png"',
                        '"source":"../../'+ship+'-reference-v1/'+ship+'-reference.png","detailLevel":"low-poly authored mesh"')
    (out/'mesh_core.py').write_text(core)
    helpers = (template/'geometry.py').read_text()
    helpers = helpers.replace('for the Scout.', 'for the pirate fleet.').replace('segments=48', 'segments=12').replace('segments=32', 'segments=12')
    (out/'geometry.py').write_text(helpers)
    shutil.copyfile(ROOT/'prepare_textures.py', out/'prepare_textures.py')
    for name in ['build_model.py', 'designs.py']:
        shutil.copyfile(ROOT/name, out/name)
    (out/'requirements.txt').write_text('numpy>=2.0\nPillow>=10.0\n')
    prompts = json.loads((ROOT/'prompts.json').read_text())
    p = next(p for p in prompts['ships'] if p['id']==ship)
    (out/'source/prompts.json').write_text(json.dumps(p,indent=2)+'\n')
    if Path(p['input']).exists():
        shutil.copyfile(p['input'], out/'source/original-game-sprite.png')
    print('Scaffolded', ship)

if __name__=='__main__':
    for ship in SHIPS: scaffold(ship)
