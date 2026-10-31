"""Combat Freighter: closed, reference-textured geometry in the fleet pipeline."""
from pathlib import Path
import copy
import hashlib
import json
import math
import sys

import numpy as np

ROOT=Path(__file__).resolve().parent
sys.path.insert(0,str(ROOT.parent/'mining-fleet-v1'))
sys.path.insert(0,str(ROOT.parent/'trader-fleet-v1'))
from reconstruction import Reconstruction
from texture_mapping import TextureMapping
from pipeline_geometry import unit,triangulate

SPEC=dict(id='combat-freighter',name='Combat Freighter',center=[267,440],pixelsPerUnit=[38,38],
    reference='combat-freighter-reference-v1/combat-freighter-reference.png',
    sprite='combat-freighter-reference-v1/combat-freighter.png',triangleBudget=[5000,6000],
    counts=dict(cargoCitadels=1,holdDoors=2,grappleArms=4,grappleFingers=12,mainEngines=3,guns=0),
    regions=dict(top=[48,51,474,832],bottom=[498,34,924,828],
        front=[1040,69,1620,290],back=[1035,344,1623,557],side=[900,600,1740,825]),
    sideRegistration=[[51,290,610,690,832],[896,1106,1530,1622,1738]],
    sideBaseline=744,sideHeightScale=40,
    patches=dict(cyan=[263,102,271,141],glass=[243,180,293,188]))


class CombatFreighter(TextureMapping,Reconstruction):
    plan_uv=Reconstruction.plan_uv

    def mesh(self,name,vertices,faces,style='auto',mapping=None,smooth=False,assembly='hull'):
        if mapping is None and style=='auto' and name.startswith(('hull.','cargo.citadel')):
            vertices,faces=self.split_registration_planes(vertices,faces)
        self._chart_vertices=np.asarray(vertices,float)
        if mapping is None:
            def mapping(p,n,fi):
                if style!='auto':return self.face_uv(p,n,style)
                if abs(n[2])>.20:return self.plan_uv(p,n[2]<0)
                if np.ptp(self._chart_vertices[:,2])<=.36:
                    direction=unit(np.array([n[0],-n[1]]))
                    return self.plan_uv(p)-(p[:,2].max()-p[:,2])[:,None]*38*direction/2048
                if abs(n[0])>.55 and name.startswith(('hull.','cargo.citadel')):
                    py=self.spec['center'][1]-p[:,1]*38
                    u=np.interp(py,*self.spec['sideRegistration'])
                    # Use the clean lower armor band for cargo walls. The
                    # upper elevation includes grapple arms, which are actual
                    # geometry and must not also be painted onto the hull.
                    v=(792-p[:,2]*16 if py.min()>=290-1e-5 else
                       self.spec['sideBaseline']-p[:,2]*self.spec['sideHeightScale'])
                    return np.column_stack([u,v])/2048
                return self.face_uv(p,n,'dark')
        selected=mapping
        def checked(p,n,fi):
            uv=np.asarray(selected(p,n,fi),float)
            t=uv[np.asarray(triangulate(p))]
            a,b=t[:,1]-t[:,0],t[:,2]-t[:,0]
            assert np.isfinite(uv).all() and uv.min()>=0 and uv.max()<=1,(name,fi,'UV outside atlas')
            assert np.abs(a[:,0]*b[:,1]-a[:,1]*b[:,0]).min()>1e-12,(name,fi,'Collapsed UV chart')
            return uv
        Reconstruction.mesh(self,name,vertices,faces,style,checked,smooth,assembly)

    def lathe(self,name,center,axis,rows,segments=24,style='auto',wall='dark',cap=None,ring=False):
        """Physical-density cylindrical/radial UVs; no collapsed cap mappings."""
        center=np.asarray(center,float);axis=unit(axis)
        helper=[0,1,0] if abs(axis[2])>.9 else [0,0,1]
        right=unit(np.cross(axis,helper));up=unit(np.cross(axis,right))
        vertices=[center+axis*h+r*(right*math.cos(i*math.tau/segments)+up*math.sin(i*math.tau/segments))
                  for h,r in rows for i in range(segments)]
        count=len(rows) if ring else len(rows)-1
        faces=[(k*segments+i,k*segments+(i+1)%segments,((k+1)%len(rows))*segments+(i+1)%segments,
                ((k+1)%len(rows))*segments+i) for k in range(count) for i in range(segments)]
        if not ring:faces += [tuple(reversed(range(segments))),tuple(range(len(vertices)-segments,len(vertices)))]
        rmax=max(r for _,r in rows);hmin=rows[0][0];hmax=max(h for h,_ in rows)
        rect=np.asarray(self.spec['patches'][wall],float).reshape(2,2);mid=rect.mean(0)
        scale=min(38,float((rect[1,0]-rect[0,0]-8)/(math.tau*rmax)),float((rect[1,1]-rect[0,1]-8)/max(hmax-hmin,1e-8)))
        radial_scale=min(38,float(np.min(rect[1]-rect[0]-8)/(2*rmax)))
        def mapping(p,n,fi):
            local=p-center;radial=np.column_stack([local@right,-local@up])
            is_cap=not ring and fi>=count*segments
            if is_cap and fi==count*segments+1 and cap:
                cx,cy,pr=cap
                return (np.array([cx,cy])+radial/rows[-1][1]*pr)/2048
            if is_cap or abs(n@axis)>.8:return (mid+radial*radial_scale)/2048
            if style=='auto' and name.endswith('.housing') and abs(n[2])>.65:
                return self.plan_uv(p,n[2]<0)
            angles=np.unwrap(np.arctan2(local@up,local@right))
            if angles.min() < -1e-7:angles+=math.tau
            return (mid+np.column_stack([(angles-math.pi)*rmax,(hmin+hmax)/2-local@axis])*scale)/2048
        self.mesh(name,vertices,faces,style,mapping,smooth=True)


def loft(m,name,stations):
    vertices=[]
    for y,width,low,high in stations:
        bevel=min(.19,(high-low)/3)
        for dx,z in [(-width*.86,high),(width*.86,high),(width,high-bevel),
                     (width,low+bevel),(width*.86,low),(-width*.86,low),
                     (-width,low+bevel),(-width,high-bevel)]:
            vertices.append(m.point(267+dx,y,z))
    n=len(stations)
    faces=[tuple(reversed(range(8))),tuple(range((n-1)*8,n*8))]
    faces += [(k*8+i,k*8+(i+1)%8,(k+1)*8+(i+1)%8,(k+1)*8+i)
              for k in range(n-1) for i in range(8)]
    m.mesh(name,vertices,faces)


def build(m):
    p=m.point
    loft(m,'hull.armored-wedge-and-keel',[(52,10,-.12,.13),(115,40,-.32,.56),
        (181,68,-.52,1.02),(243,108,-.69,1.25),(289,141,-.80,1.18),
        (368,155,-.83,.95),(572,166,-.83,.95),(653,190,-.70,.82),
        (704,169,-.57,.71),(723,122,-.46,.57)])
    m.box('cargo.citadel',(141,291,392,617),.75,1.61,bevel=.14,corner=24)
    # Two separate clamshell leaves meet at the narrow, recessed centre seam.
    m.box('cargo.hold-door.port',(173,297,262,607),1.56,1.78,bevel=.06,corner=10)
    m.box('cargo.hold-door.starboard',(271,297,360,607),1.56,1.78,bevel=.06,corner=10)
    m.box('cargo.centre-seal',(263,300,270,604),1.57,1.67,'dark',.025,2)
    for side,label in [(-1,'port'),(1,'starboard')]:
        mirror=lambda x:267+side*(x-267)
        for i,y in enumerate([329,439,547]):
            x=mirror(372)
            m.beam(f'cargo.hinge.{label}.{i}',p(x,y,1.355),p(x,y+35,1.355),8/38,.41,'brass')
        for i,y in enumerate([342,556]):
            x=mirror(272)
            m.beam(f'cargo.latch.{label}.{i}',p(x,y,1.82),p(x,y+22,1.82),8/38,.10,'brass')
        outline=[(mirror(x),y) for x,y in [(383,298),(414,335),(428,606),(449,663),
            (426,713),(382,679),(367,613),(373,343)]]
        m.slab(f'armor.{label}.angled-skirt',outline,.56,1.16,.10)
        for number,(root_y,wrist_y) in enumerate([(400,300),(599,499)],1):
            # Each claw is a mechanical grab: three bent fingers, no barrels.
            tag=f'grapple.{label}.{number}'
            root=p(mirror(447),root_y,1.03)
            elbow=p(mirror(424),root_y-42,1.20)
            wrist=p(mirror(397),wrist_y+10,1.28)
            m.box(tag+'.mount',(min(mirror(420),mirror(474)),root_y-13,max(mirror(420),mirror(474)),root_y+31),.48,.99,'dark',.08,10)
            m.beam(tag+'.arm.root',root,elbow,.38,.29,'auto')
            m.beam(tag+'.arm.fore',elbow,wrist,.31,.24,'auto')
            m.cylinder(tag+'.hydraulic',root+[-side*.16,0,.17],wrist+[-side*.16,0,.17],.105,'silver',12)
            for joint,q in enumerate([root,wrist]):
                px=267+q[0]*38;py=440-q[1]*38
                m.lathe(tag+f'.pivot.{joint}',q,[0,0,1],[(0,.29),(.08,.35),(.23,.28)],12,'dark','dark',cap=(px,py,10))
                m.lathe(tag+f'.pin.{joint}',q+[0,0,.24],[0,0,1],[(0,.18),(.05,.18)],12,'brass','brass')
            palm=p(mirror(415),wrist_y+12,1.32)
            m.beam(tag+'.palm',wrist,palm,.36,.27,'silver')
            for finger,offset in enumerate([-13,0,13],1):
                base=p(mirror(415+offset),wrist_y+17,1.31)
                knee=p(mirror(420+offset*1.12),wrist_y-5,1.35)
                tip=p(mirror(415+offset*.88),wrist_y-19,1.28)
                m.beam(tag+f'.finger.{finger}.root',base,knee,.13,.16,'silver')
                m.beam(tag+f'.finger.{finger}.tip',knee,tip,.115,.14,'dark')
    # Narrow slit integrated into the bow shoulder, not a large glass canopy.
    m.box('bridge.armored-slit',(235,177,299,189),.97,1.095,bevel=.025,corner=4)
    m.box('hull.stern-engine-block',(116,637,418,718),-.63,.68,bevel=.12,corner=17)
    for label,cx,y0,y1,radius,cap in [
        ('port',153,675,815,1.20,(1228,484,41)),
        ('centre',267,657,832,1.46,(1332,484,55)),
        ('starboard',379,675,815,1.20,(1438,484,41))]:
        c=p(cx,y0,0);length=(y1-y0)/38
        m.lathe(f'engine.{label}.housing',c,[0,-1,0],[(0,radius*.66),(.22,radius*.90),
            (.50,radius),(length*.58,radius),(length*.66,radius*.95),
            (length*.77,radius*.80),(length*.95,radius*.80),(length,radius*.72)],24,'auto','dark',cap=cap)
        m.lathe(f'engine.{label}.collar',c,[0,-1,0],[(length*.54,radius*1.025),
            (length*.62,radius*1.025),(length*.62,radius*.96),(length*.54,radius*.96)],20,'brass','brass',ring=True)
        m.lathe(f'engine.{label}.nozzle-rim',c,[0,-1,0],[(length*.94,radius*.82),
            (length+.035,radius*.82),(length+.035,radius*.64),(length*.94,radius*.64)],24,'dark','dark',ring=True)
        m.lathe(f'engine.{label}.exhaust',c,[0,-1,0],[(length-.015,radius*.64),
            (length+.005,radius*.64)],24,'dark','dark',cap=(cap[0],cap[1],cap[2]*.88))
        m.socket('socket.engine.'+label,c+[0,-length-.05,0],[0,-1,0],'engine-exhaust')
        m.box(f'armor.{label}.engine-shield',(cx-21,y0+8,cx+21,y0+54),radius*.87,radius+.11,bevel=.04,corner=5)
    m.box('ventral.loading-hatch',(204,316,331,558),-1.02,-.78,bevel=.065,corner=13)
    for side,label in [(-1,'port'),(1,'starboard')]:
        x=267+side*91
        for i,y in enumerate([338,564]):
            m.box(f'ventral.{label}.service-panel.{i}',(x-20,y,x+20,y+59),-.94,-.78,bevel=.045,corner=5)
    for i,y in enumerate([207,600]):
        m.box(f'ventral.keel-access.{i}',(240,y,294,y+42),-.90,-.74,bevel=.04,corner=6)


def build_one():
    m=CombatFreighter(ROOT,copy.deepcopy(SPEC))
    m.prepare_textures();build(m);m.export()
    info=json.loads((ROOT/'model-info.json').read_text())
    info.update(role='Unarmed raiding cargo carrier',geometry='One armored cargo citadel, two clamshell doors, four folded three-finger grapple arms, three level engines.',
        materials='Native reference projections and existing fleet metal trims, three embedded 2048px PBR maps. Amber is paint; only cyan drives emission.',
        axes='Y up, -Z forward',weaponSockets=0,uvFallbackPolicy='Invalid UV charts fail the build.',
        stage='Static source model; production uses the verified LOD2 export.',
        note='Orthographic concept reconstruction. Production integration supplies dedicated map/panel art and three engine hardpoints; gameplay rules are unchanged.')
    (ROOT/'model-info.json').write_text(json.dumps(info,indent=2)+'\n')
    paths=[ROOT.parents[1]/SPEC['reference'],ROOT.parents[1]/SPEC['sprite'],ROOT.parent/'mining-fleet-v1/material-strip.png']
    (ROOT/'source-integrity.json').write_text(json.dumps({'sources':[{'path':str(p),'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in paths],
        'handling':'Read-only reference pixels packed at native scale; source images copied unchanged.'},indent=2)+'\n')


if __name__=='__main__':build_one()
