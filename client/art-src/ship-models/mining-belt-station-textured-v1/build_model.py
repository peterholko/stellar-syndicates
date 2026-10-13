"""Reconstruct the six-silo Mining Belt Station, Y-up, front=-Z; no Blender."""
from pathlib import Path
import math
import sys
import numpy as np
ROOT=Path(__file__).resolve().parent
sys.path.insert(0,str(ROOT.parent/'mining-fleet-v1'))
from reconstruction import Reconstruction
from pipeline_geometry import unit

SPEC=dict(id='mining-belt-station',name='Mining Belt Station',center=[292,455],pixelsPerUnit=[33.5,40],
    reference='mining-belt-station-reference-v1/mining-belt-station-reference.png',
    sprite='mining-belt-station-reference-v1/mining-belt-station.png',triangleBudget=[10000,16000],
    counts={'oreSilos':6,'bargeBerths':2,'haulerGantries':1,'anchoringBooms':4,'stationKeepingPods':4,'mainEngines':0,'guns':0},
    regions={'top':[24,91,552,753],'bottom':[557,90,1068,753],
             'front':[1085,120,1720,329],'back':[1085,382,1720,588],'side':[1086,631,1725,834]},
    patches={'silver':[1258,174,1287,210],'dark':[285,250,300,330],
             'brass':[287,185,294,208],'hazard':[278,151,305,195],
             'cyan':[256,448,261,462],'glass':[728,343,740,356],
             'silowall':[1248,154,1343,215]})


def build(m):
    p=m.point
    core=[(262,389),(322,389),(356,423),(356,486),(322,520),(262,520),(227,486),(227,423)]
    m.slab('core.octagonal-foundation',core,-.63,.44,.10)
    m.slab('core.octagonal-armored-deck',core,.42,.99,.1)
    m.box('core.ventral-maintenance-hatch',(260,424,323,489),-.85,-.56,bevel=.07)
    m.lathe('core.central-service-cap',p(292,455),[0,0,1],[(.95,.83),(1.07,1.12),(1.32,1.12),(1.43,.88)],32)
    # Six load-bearing spokes terminate at six upright silos in a radial cluster.
    silos=[(208,347),(176,455),(208,576),(376,347),(409,455),(376,576)]
    for i,(x,y) in enumerate(silos):
        name=f'silo.{i+1}'
        center=p(x,y)
        axis=unit(p(x,y)-p(292,455))
        m.beam(name+'.foundation-spoke',p(292,455,-.28),center+[0,0,-.28],.95,.66,'dark')
        m.cylinder(name+'.ore-transfer-pipe',p(292,455,.05),center+[0,0,.05],.31,'silver',16)
        # Rounded shoulders and closed top/bottom caps. All six walls share the
        # reference elevation's tank plating through continuous angular UVs.
        m.lathe(name+'.pressure-vessel',center,[0,0,1],[(-.64,1.17),(-.52,1.38),(-.32,1.49),
            (1.43,1.49),(1.65,1.38),(1.85,1.08),(1.92,.85)],40,'auto','silver')
        for j,z in enumerate((-.30,1.42)):
            m.lathe(name+f'.retaining-band.{j}',center,[0,0,1],[(z,1.50),(z+.10,1.50),(z+.10,1.44),(z,1.44)],40,'dark','dark',ring=True)
        m.lathe(name+'.top-service-hatch',center,[0,0,1],[(1.90,.42),(1.98,.42)],24)
        m.lathe(name+'.bottom-service-hatch',center,[0,0,1],[(-.70,.48),(-.62,.48)],24)
        for j in range(4):
            angle=j*math.tau/4
            q=center+np.array([math.cos(angle)*1.47,math.sin(angle)*1.47,0])
            m.beam(name+f'.reinforcing-strap.{j}',q+[0,0,-.31],q+[0,0,1.43],.075,.09,'brass')
        # The upper and lower clamps remain textured from all directions.
        for j,dy in enumerate((-43,43)):
            m.box(name+f'.cap-clamp.{j}',(x-4,y+dy-5,x+4,y+dy+5),1.48,1.80,'brass',.025,2)
    # The two barge berths are real empty windows, enclosed by four slim rails.
    for side,label in ((-1,'port'),(1,'starboard')):
        cx=73 if side<0 else 508
        innerx=122 if side<0 else 458
        startx=176 if side<0 else 409
        m.cylinder(f'berth.{label}.neck',p(startx,455,.08),p(innerx,455,.08),.51,'dark',20)
        for y in (439,474):
            m.cylinder(f'berth.{label}.transfer-pipe.{y}',p(startx,y,.21),p(innerx,y,.21),.105,'silver',12)
        m.box(f'berth.{label}.outer-rail',(cx-45,412,cx-33,503),-.34,.41,bevel=.085)
        m.box(f'berth.{label}.inner-rail',(cx+33,412,cx+45,503),-.34,.41,bevel=.085)
        for y in (412,491):
            m.box(f'berth.{label}.cross-rail.{y}',(cx-39,y,cx+39,y+12),-.34,.41,bevel=.07)
            m.box(f'berth.{label}.hazard.{y}',(cx-27,y+2,cx+27,y+10),.405,.455,'hazard',.012,2)
        for x in (cx-36,cx+36):
            for y in (423,486):
                m.box(f'berth.{label}.clamp.{x}.{y}',(x-5,y-5,x+5,y+5),.35,.63,'brass',.04,2)
        m.socket('socket.docking.barge.'+label,p(cx,455,.65),[side,0,0],'barge-berth',
                 clearance={'width':1.52,'length':1.92,'approach':'from above or laterally; art berth is not a full ship bounding box'})
    # One forward covered conveyor, a rectangular hauler head and aft service spine.
    m.box('gantry.forward-spine',(273,115,310,403),-.34,.10,bevel=.045)
    m.box('gantry.covered-conveyor',(282,178,301,383),.10,.38,'dark',.045)
    for x in (275,306):
        m.beam(f'gantry.side-rail.{x}',p(x,169,.39),p(x,387,.39),.095,.10,'brass')
    for y in (214,259,306,355):
        m.box(f'gantry.cover-segment.{y}',(280,y,302,y+32),.36,.43,'dark',.024)
    m.box('gantry.hauler-loading-head',(252,102,333,212),-.36,.72,bevel=.12,corner=10)
    m.box('gantry.hauler-hazard-panel',(280,129,305,202),.718,.755,'hazard',.012)
    m.box('gantry.forward-coupler',(279,92,307,113),-.10,.45,bevel=.06)
    m.socket('socket.docking.hauler',p(292,87,.2),[0,1,0],'hauler-loading-berth')
    m.box('core.aft-service-spine',(268,517,315,717),-.57,.20,bevel=.07)
    m.box('core.aft-service-head',(258,673,325,739),-.51,.45,bevel=.10)
    for y in (538,601):
        m.box(f'ventral.spine-hatch.{y}',(277,y,309,y+42),-.71,-.53,bevel=.04)
    # Exactly four diagonal booms, each open truss ending in a station-keeping pod.
    for i,(x0,y0,x1,y1) in enumerate(((166,311,101,258),(418,311,483,258),
                                      (166,609,101,660),(418,609,483,660))):
        a,b=p(x0,y0,-.35),p(x1,y1,-.75)
        delta=unit(b-a);right=unit(np.cross(delta,[0,0,1]))*.22
        m.cylinder(f'anchor.{i}.root-joint',a+[0,0,-.2],a+[0,0,.45],.44,'brass',20)
        for sign in (-1,1):
            m.beam(f'anchor.{i}.truss-rail.{sign}',a+right*sign,b+right*sign,.10,.12,'dark')
        for j in range(4):
            u=a+(b-a)*(j/4);v=a+(b-a)*((j+1)/4)
            m.beam(f'anchor.{i}.cross-brace.{j}',u+right*((j%2)*2-1),v-right*((j%2)*2-1),.072,.08,'silver')
        m.box(f'anchor.{i}.keeper-pod',(x1-18,y1-20,x1+18,y1+20),-1.10,-.40,bevel=.075,corner=8)
        m.box(f'anchor.{i}.ochre-clamp',(x1-10,y1-6,x1+10,y1+6),-.42,-.32,'brass',.024)
        # Very small maneuvering jets only. No bank of main drive engines.
        for side in (-1,1):
            q=b+right*side*1.40+[0,0,-.12]
            m.cylinder(f'anchor.{i}.maneuvering-jet.{side}',q,q+right*side*.38,.12,'dark',10)
        m.socket(f'socket.anchor.{i+1}',b+[0,0,-.38],[0,0,-1],'anchoring-boom')
    # Visible underframe and service pipes connect tanks without sealing gaps.
    for i in range(3):
        y=(347,455,576)[i]
        m.beam(f'ventral.cross-tie.{i}',p(196,y,-.64),p(388,y,-.64),.11,.12,'dark')
    for side in (-1,1):
        x=292+side*84
        for j,(a,b) in enumerate(((360,415),(494,560))):
            m.cylinder(f'coolant.main.{side}.{j}',p(x,a,.17),p(x,b,.17),.13,'silver',12)
    # Floodlight brackets are compact, textured fittings, not new silhouette arms.
    for i,(x,y) in enumerate(((255,386),(331,386),(255,523),(331,523))):
        m.box(f'floodlight.{i}.mount',(x-5,y-5,x+5,y+5),.35,.64,'dark',.03)
        m.box(f'floodlight.{i}.lens',(x-3,y-3,x+3,y+3),.64,.67,'cyan',.008,1)


if __name__=='__main__':
    model=Reconstruction(ROOT,SPEC)
    model.prepare_textures();build(model);model.export()
