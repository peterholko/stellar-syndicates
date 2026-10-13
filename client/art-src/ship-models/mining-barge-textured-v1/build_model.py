"""Reconstruct the Mining Barge from its five-view sheet; no Blender."""
from pathlib import Path
import math
import sys
import numpy as np
ROOT=Path(__file__).resolve().parent
sys.path.insert(0,str(ROOT.parent/'mining-fleet-v1'))
from reconstruction import Reconstruction

SPEC=dict(id='mining-barge',name='Mining Barge',center=[270,444],pixelsPerUnit=[31,40],
    reference='mining-barge-reference-v1/mining-barge-reference.png',
    sprite='mining-barge-reference-v1/mining-barge.png',triangleBudget=[5000,8000],
    counts={'miningArms':2,'cutterEmitters':2,'coveredHoppers':1,'bridges':1,'mainEngines':2,'guns':0},
    regions={'top':[44,59,496,827],'bottom':[550,59,991,827],
             'front':[1055,102,1695,281],'back':[1058,336,1695,560],'side':[975,599,1733,820]},
    patches={'silver':[249,303,268,341],'dark':[235,482,305,628],
             'brass':[266,388,283,412],'hazard':[78,210,112,255],
             'cyan':[111,123,117,138],'glass':[110,422,141,447],
             'enginewall':[1660,715,1718,769]})


def build(m):
    p=m.point
    m.slab('hull.armored-keel',[(111,337),(171,308),(355,308),(425,352),(451,409),
        (443,615),(405,671),(351,718),(318,789),(228,789),(198,716),(131,669),(91,602),(88,411)],-.75,.65,.13)
    m.slab('hull.shoulder-deck',[(125,340),(187,305),(356,307),(415,350),(429,532),
        (404,651),(337,695),(210,695),(130,637),(110,516)],.52,1.00,.13)
    m.box('hull.bow-crossmember',(120,260,420,325),-.28,.91,bevel=.11)
    m.box('crusher.intake-foundation',(184,164,354,299),.26,.91,bevel=.1)
    m.box('crusher.dark-throat',(214,184,327,282),.87,1.01,'dark',.045)
    for index in range(5):
        x=222+index*21
        m.cylinder(f'crusher.feed-roller.{index}',p(x,191,1.19),p(x,274,1.19),.16,'dark',12)
        for y in (196,222,250,268):
            m.beam(f'crusher.tooth.{index}.{y}',p(x-5,y,1.295),p(x+5,y,1.295),.15,.15,'dark')
    for x in (188,332):
        m.box(f'crusher.intake-rail.{x}',(x,165,x+21,289),.89,1.45,bevel=.075)
    m.box('crusher.front-lip',(210,166,327,184),1.13,1.38,bevel=.05)
    m.box('hull.control-cap',(207,286,336,386),.93,1.33,bevel=.11,corner=19)
    m.box('hopper.covered-single-bay',(203,419,334,667),.95,1.82,bevel=.18,corner=14)
    for i in range(10):
        y=438+i*22
        m.box(f'hopper.reinforcing-rib.{i}',(210,y,328,y+5),1.80,1.91,'dark',.025,2)
    for x in (197,334):
        m.box(f'hopper.retaining-rail.{x}',(x,434,x+8,646),1.03,1.51,'brass',.035,2)
    for x in (163,354):
        m.box(f'hull.longitudinal-side-armor.{x}',(x,508,x+31,649),.8,1.20,bevel=.06)
    m.box('hull.aft-spine',(229,668,318,786),.45,1.02,bevel=.1,corner=9)
    # The bridge occupies port only. Its glazing is sampled from the port cabin.
    m.slab('bridge.port-cabin',[(96,395),(117,361),(150,361),(171,393),
        (169,491),(144,527),(112,510),(94,472)],.78,1.92,.16)
    m.box('bridge.port-glazing',(107,414,153,450),1.921,1.948,'glass',.008,4)
    m.lathe('bridge.antenna-base',p(124,365,1.7),[0,0,1],[(0,.18),(.32,.18),(.44,.12)],12,'dark','dark')
    m.cylinder('bridge.antenna',p(124,365,2.12),p(124,365,3.03),.043,'dark',8)
    # Side-by-side tool arms stay short and rigid; sockets are at the emitter tips.
    for side,label in ((-1,'port'),(1,'starboard')):
        mirror=lambda x:270+side*(x-270)
        cx=mirror(418);joint=mirror(408)
        outline=[(mirror(x),y) for x,y in [(368,242),(399,174),(419,148),(462,179),(471,231),(439,279),(403,288)]]
        m.slab(f'arm.{label}.shell',outline,.24,1.13,.09)
        m.lathe(f'arm.{label}.pivot',p(joint,270),[0,0,1],[(.50,.69),(.60,.82),(.98,.82),(1.13,.68),(1.22,.47)],24,'auto','brass')
        m.cylinder(f'arm.{label}.hydraulic',p(mirror(458),276,.21),p(mirror(469),223,.68),.18,'dark',12)
        m.cylinder(f'arm.{label}.hydraulic-rod',p(mirror(458),276,.21),p(mirror(435),303,.12),.08,'brass',10)
        center=p(cx,150,1.16)
        m.cylinder(f'arm.{label}.cutter-neck',p(cx,184,.81),p(cx,148,1.16),.48,'dark',12)
        m.lathe(f'cutter.{label}.barrel',center,[0,1,0],[(0,.80),(.23,1.08),(.48,1.22),
            (1.13,1.22),(1.28,1.35),(1.62,1.35),(1.78,1.16),(2.05,1.16)],32,'auto','enginewall',cap=(1121,195,54))
        m.lathe(f'cutter.{label}.brass-collar',center,[0,1,0],[(1.40,1.37),(1.57,1.37),(1.57,1.25),(1.40,1.25)],32,'brass','brass',ring=True)
        # A recessed luminous lens is idle; no beam or long exhaust geometry.
        m.lathe(f'cutter.{label}.emitter-face',center,[0,1,0],[(2.045,.84),(2.083,.84)],24,'dark','dark',cap=(1121,195,38))
        for i in range(8):
            a=i*math.tau/8
            q=center+np.array([math.cos(a)*1.13,2.00,math.sin(a)*1.13])
            m.cylinder(f'cutter.{label}.rim-fastener.{i}',q,q+[0,.10,0],.07,'dark',6)
        m.socket('socket.mining.'+label,center+[0,2.10,0],[0,1,0],'mining-beam',localForward='-Z')
        # Small machinery cans flank the hopper; these are not separate holds.
        m.lathe(f'hull.{label}.coolant-reservoir',p(mirror(402),490,1.0),[0,1,0],[(0,.43),(.1,.52),(1.82,.52),(1.95,.42)],16,'silver','silver')
        m.box(f'hull.{label}.aft-armor',(min(mirror(357),mirror(414)),535,max(mirror(357),mirror(414)),675),.43,1.18,bevel=.08)
        ex=mirror(382);ec=p(ex,707,.34)
        m.cylinder(f'engine.{label}.mount',p(ex,659,.34),ec,.65,'dark',12)
        m.lathe(f'engine.{label}.housing',ec,[0,-1,0],[(0,.69),(.18,.94),(.51,1.05),
            (1.69,1.05),(1.95,.95),(2.20,.80),(2.52,.83),(2.67,.70)],28,'auto','enginewall',cap=(1286,477,54))
        m.lathe(f'engine.{label}.nozzle-rim',ec,[0,-1,0],[(2.48,.83),(2.72,.83),(2.72,.65),(2.48,.65)],28,'dark','enginewall',ring=True)
        m.lathe(f'engine.{label}.exhaust',ec,[0,-1,0],[(2.53,.64),(2.56,.64)],24,'dark','dark',cap=(1286,477,39))
        m.socket('socket.engine.'+label,ec+[0,-2.75,0],[0,-1,0],'engine-exhaust')
        for y in (412,581):
            x=mirror(463)
            m.box(f'hull.{label}.side-hazard.{y}',(x-8,y,x+8,y+53),.12,.57,'hazard',.06)
        # Ventral access panels use only the dedicated underside drawing.
        for y in (390,592):
            x=mirror(372)
            m.box(f'ventral.{label}.hatch.{y}',(x-24,y,x+23,y+49),-.87,-.69,bevel=.035)
    for i,y in enumerate((343,453,527,710)):
        m.box(f'ventral.center-hatch.{i}',(235,y,311,y+37),-.94,-.72,bevel=.035)
    for x in (201,339):
        m.beam(f'ventral.spine.{x}',p(x,360,-.86),p(x,679,-.86),.09,.1,'dark')


if __name__=='__main__':
    model=Reconstruction(ROOT,SPEC)
    model.prepare_textures();build(model);model.export()
