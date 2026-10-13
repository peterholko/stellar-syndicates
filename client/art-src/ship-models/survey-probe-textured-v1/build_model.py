"""Survey Probe reconstruction from its five-view reference. No Blender.

Author coordinates use X/Y for the sheet plan and Z for height. The common
mining-fleet exporter produces Y-up, -Z-forward GLB and OBJ assets.
"""
from pathlib import Path
import sys
import math
import numpy as np

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT.parent / 'mining-fleet-v1'))
from reconstruction import Reconstruction

SPEC = dict(
    id='survey-probe', name='Survey Probe', center=[288, 418],
    pixelsPerUnit=[112, 120],
    reference='survey-probe-reference-v1/survey-probe-reference.png',
    sprite='survey-probe-reference-v1/survey-probe.png',
    triangleBudget=[1500, 3000],
    counts=dict(sensorDishes=1, antennaBooms=4, radiatorFins=2,
                mainEngines=1, guns=0, cockpits=0, crewModules=0),
    regions={'top':[38, 61, 536, 772], 'bottom':[587, 61, 1085, 772],
             'front':[1173, 64, 1726, 289], 'back':[1173, 339, 1726, 592],
             'side':[1178, 620, 1730, 812]},
    patches={'silver':[233, 388, 251, 434], 'dark':[274, 543, 300, 598],
             'brass':[256, 658, 318, 667], 'cyan':[1448, 158, 1457, 168],
             'radiator':[1284, 184, 1320, 224], 'enginewall':[1580, 712, 1627, 757]})


class Probe(Reconstruction):
    def prepare_textures(self):
        # Use the established atlas packing and matching ImageGen material strip.
        # Tiny brass hardware uses this craft's native engine-collar sample.
        brass = list(self.spec['patches']['brass'])
        super().prepare_textures()
        self.spec['patches']['brass'] = brass

    def mesh(self,name,vertices,faces,style='auto',mapping=None,smooth=False,assembly='hull'):
        # Preserve the native side elevation on hull walls instead of repeating
        # a generic strip of silver plating across every narrow side face.
        if mapping is None and style=='auto' and name.startswith(('hull.','ventral.')):
            def mapping(p,normal,fi):
                if abs(normal[2])>.55:
                    return self.plan_uv(p,normal[2]<0)
                if abs(normal[0])>.55:
                    return np.column_stack([1401.6-p[:,1]*92.4,730-p[:,2]*150])/2048
                return self.face_uv(p,normal,'dark')
        super().mesh(name,vertices,faces,style,mapping,smooth,assembly)


def dish(m):
    """A real concave forward-facing bowl with wall thickness and an optical eye."""
    center=m.point(288, 309, .11)
    n=24
    rows=[(0, .24), (.20, .52), (.55, .94), (.64, .96),
          (.66, .91), (.44, .69), (.20, .18), (.055, .18)]
    vertices=[center+[radius*math.cos(i*math.tau/n), h, radius*math.sin(i*math.tau/n)]
              for h,radius in rows for i in range(n)]
    faces=[(k*n+i,k*n+(i+1)%n,((k+1)%len(rows))*n+(i+1)%n,((k+1)%len(rows))*n+i)
           for k in range(len(rows)) for i in range(n)]
    def mapping(p, normal, fi):
        if fi//n >= 3:
            local=p-center
            # Projection of the dedicated FRONT drawing into the bowl itself.
            return np.column_stack([1452+local[:,0]/.96*95,
                                    164-local[:,2]/.96*95])/2048
        if abs(normal[2])>.55:
            return m.plan_uv(p, normal[2]<0)
        return m.face_uv(p,normal,'dark')
    m.mesh('sensor.forward-dish',vertices,faces,'auto',mapping=mapping,smooth=True)
    m.lathe('sensor.optical-eye',center+[0,.18,0],[0,1,0],
            [(0,.18),(.10,.24),(.16,.23)],16,'dark','dark',cap=(1452,164,24))
    # Four small brass rim clamps are fittings, not antennae or extra sensors.
    for i in range(4):
        a=i*math.tau/4
        c=center+[math.cos(a)*.91,.64,math.sin(a)*.91]
        m.beam(f'sensor.rim-clamp.{i+1}',c+[0,-.055,0],c+[0,.065,0],.13,.085,'brass')


def folded_fin(m, side, label):
    x0,x1=(129,177) if side<0 else (399,447)
    # Outer edge raised slightly: the paired panel fins are genuinely folded.
    outline=[(x0,339),(x0+8,331),(x1,348),(x1,480),(x0+9,497),(x0,488)]
    xy=np.array([m.point(x,y)[:2] for x,y in outline])
    xs=np.array([x for x,y in outline]);outer=x0 if side<0 else x1
    height=.08+.20*(1-np.abs(xs-outer)/(x1-x0))
    vertices=[np.r_[xy[i],height[i]+offset] for offset in (-.055,.055) for i in range(6)]
    faces=[tuple(reversed(range(6))),tuple(range(6,12))]
    faces += [(i,(i+1)%6,(i+1)%6+6,i+6) for i in range(6)]
    m.mesh(f'radiator.{label}.folded-panel',vertices,faces,'auto')
    x=186 if side<0 else 389
    m.cylinder(f'radiator.{label}.hinge',m.point(x-8,407,.11),m.point(x+8,407,.11),.105,'brass',8)


def build(m):
    p=m.point
    outline=[(222,334),(256,324),(320,324),(351,340),(378,372),
             (376,548),(329,621),(254,621),(204,565),(195,373)]
    m.slab('hull.armored-core',outline,-.29,.28,.055)
    m.slab('hull.dorsal-plates',[(224,338),(257,337),(318,337),(350,353),
           (355,550),(319,616),(257,616),(218,551)],.24,.40,.04)
    m.slab('ventral.service-plating',[(224,340),(351,340),(365,377),
           (361,548),(321,615),(255,615),(211,547),(209,375)],-.365,-.27,.025)
    for side,label in ((-1,'port'),(1,'starboard')):
        mirror=lambda x:288+side*(x-288)
        panel=[(mirror(x),y) for x,y in [(352,367),(376,383),(376,535),(355,569),(346,531),(346,390)]]
        m.slab(f'hull.{label}.cheek',panel,-.12,.30,.025)
        folded_fin(m,side,label)
    m.lathe('sensor.neck',p(288,338,.11),[0,1,0],[(0,.36),(.14,.34),(.29,.25)],12,'auto','dark')
    dish(m)
    # Exactly four articulated-looking but static antenna booms, no gun barrels.
    # Forward arms rise and rear arms dip slightly, visible in end elevations.
    for side,label in ((-1,'port'),(1,'starboard')):
        mirror=lambda x:288+side*(x-288)
        for forward,end in ((True,'forward'),(False,'aft')):
            a=p(mirror(346 if forward else 356),337 if forward else 537,.16 if forward else -.13)
            b=p(mirror(516 if forward else 524),74 if forward else 759,.46 if forward else -.34)
            d=b-a
            stem=f'antenna.{label}.{end}'
            m.cylinder(stem+'.boom',a,b,.042,'dark',8)
            for suffix,t0,t1,r,material in (
                ('root-collar',0,.11,.095,'brass'),
                ('silver-sleeve',.53,.65,.080,'silver'),
                ('sleeve-band',.64,.68,.085,'brass'),
                ('tip-housing',.90,1,.062,'brass')):
                m.cylinder(stem+'.'+suffix,a+d*t0,a+d*t1,r,material,8)
            c0=a+d*.90+np.array([0,0,.057]);c1=a+d*.973+np.array([0,0,.057])
            m.beam(stem+'.cyan-indicator',c0,c1,.043,.012,'cyan')
    # One rear cylindrical engine with a genuinely recessed end-on aperture.
    center=p(288,620,-.015)
    m.lathe('engine.main.mount',center,[0,-1,0],[(0,.26),(.18,.26)],12,'dark','dark')
    m.lathe('engine.main.housing',center,[0,-1,0],
            [(.12,.26),(.25,.36),(.91,.36),(1.03,.30),(1.10,.29)],16,'auto','enginewall',
            cap=(1453,497,47))
    for i,h in enumerate((.31,.87)):
        m.lathe(f'engine.main.collar.{i+1}',center,[0,-1,0],
                [(h,.372),(h+.067,.372),(h+.067,.348),(h,.348)],16,'brass','brass',ring=True)
    m.lathe('engine.main.nozzle-rim',center,[0,-1,0],
            [(1.03,.31),(1.14,.31),(1.14,.243),(1.03,.243)],16,'dark','dark',ring=True)
    m.lathe('engine.main.aperture',center,[0,-1,0],[(1.07,.241),(1.088,.241)],16,
            'dark','dark',cap=(1453,497,29))
    m.socket('socket.engine.main',center+[0,-1.145,0],[0,-1,0],'engine-exhaust')
    # Raised and recessed body service details use the dedicated plan textures.
    m.lathe('hull.dorsal-service-cap',p(288,459,.40),[0,0,1],[(0,.165),(.035,.165)],12,'auto','brass')
    for i,y in enumerate((380,471,552)):
        m.box(f'ventral.access-hatch.{i+1}',(267,y,309,y+39),-.405,-.35,bevel=.009,corner=4)


if __name__=='__main__':
    model=Probe(ROOT,SPEC)
    model.prepare_textures()
    build(model)
    model.export()
