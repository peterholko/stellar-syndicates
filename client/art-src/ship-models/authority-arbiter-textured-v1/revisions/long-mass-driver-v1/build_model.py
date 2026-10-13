"""Authority Arbiter: imperial architectural interpretation of the approved sheet.

All geometry is authored with NumPy; no Blender. Plan coordinates register to
the TOP drawing. The model exports Y-up, -Z-forward with real gun-mount pivots.
"""
from pathlib import Path
import math
import numpy as np
from reconstruction import Reconstruction
from pipeline_geometry import unit

ROOT=Path(__file__).resolve().parent
SPEC=dict(id='authority-arbiter',name='Authority Arbiter',center=[225,485],pixelsPerUnit=[25.5,25.5],
    reference='authority-arbiter-reference-v1/authority-arbiter-reference.png',
    sprite='authority-arbiter-reference-v1/authority-arbiter.png',triangleBudget=[12000,22000],
    counts={'haloRings':1,'haloStruts':4,'dorsalSpines':3,'spinalLances':0,'massDrivers':1,'accelerationRails':2,
            'heavyTwinTurrets':6,'secondaryTwinTurrets':8,'pairedPointDefenseMounts':8,
            'gunBarrels':44,'mainEngines':4,'commandTowers':1,'closedVentralHangars':1},
    regions={'top':[45,140,407,795],'bottom':[485,140,847,795],
             'front':[920,83,1708,280],'back':[920,360,1708,570],'side':[861,594,1748,813]},
    patches={'amber':[1294,156,1336,169],'cyan':[1002,207,1022,216],
             'vent':[645,327,653,537]},
    emissiveAmberRegions=[[1285,149,1350,175],[1388,637,1460,658],[202,570,245,589]])


def loft(m,name,sections,center=225,style='auto'):
    """Beveled eight-sided cross-sections, with real depth and sloping shoulders."""
    vertices=[]
    for py,w,low,edge,roof in sections:
        lip=min(.14,(edge-low)*.23)
        for x,z in [(-w*.68,roof),(w*.68,roof),(w,edge),(w*.97,low+lip),
                    (w*.72,low),(-w*.72,low),(-w*.97,low+lip),(-w,edge)]:
            vertices.append(m.point(center+x,py,z))
    faces=[tuple(reversed(range(8)))]
    faces += [(k*8+i,k*8+(i+1)%8,(k+1)*8+(i+1)%8,(k+1)*8+i)
              for k in range(len(sections)-1) for i in range(8)]
    faces.append(tuple(range(len(vertices)-8,len(vertices))))
    m.mesh(name,vertices,faces,style)


def halo_sector(m,name,profile,start=0,sweep=math.tau,segments=64,style='auto'):
    """Closed annular profile; the full halo has a genuine through-opening."""
    full=abs(sweep-math.tau)<1e-8
    count=segments if full else segments+1
    vertices=[np.array([r*math.cos(start+sweep*i/segments),r*math.sin(start+sweep*i/segments),z])
              for r,z in profile for i in range(count)]
    faces=[]
    for row in range(len(profile)):
        other=(row+1)%len(profile)
        for i in range(segments):
            nxt=(i+1)%count
            faces.append((row*count+i,row*count+nxt,other*count+nxt,other*count+i))
    if not full:
        faces.extend([tuple(k*count for k in range(len(profile))),
                      tuple(k*count+segments for k in reversed(range(len(profile))))])
    rect=np.array(m.spec['patches']['silver' if style=='auto' else style]).reshape(2,2)
    r0,r1=min(r for r,z in profile),max(r for r,z in profile)
    z0,z1=min(z for r,z in profile),max(z for r,z in profile)
    def mapping(p,n,fi):
        if style=='auto' and abs(n[2])>.55:return m.plan_uv(p,n[2]<0)
        # Continuous U coordinates on each face, repeated at a face boundary.
        angle=np.unwrap(np.arctan2(p[:,1],p[:,0]))
        u=angle/math.tau*8
        u-=math.floor(float(u.min())+1e-7)
        v=(np.linalg.norm(p[:,:2],axis=1)-r0)/max(r1-r0,1e-9) if abs(n[2])>.55 else (p[:,2]-z0)/max(z1-z0,1e-9)
        return (rect[0]+np.column_stack([u,v])*(rect[1]-rect[0]))/2048
    m.mesh(name,vertices,faces,style,mapping=mapping,smooth=True)


def panel_band(m,name,x0,y0,x1,y1,z,width=.055,style='brass'):
    m.beam(name,m.point(x0,y0,z),m.point(x1,y1,z),width,width,style)


def fairing(m,name,sections,center=225,style='silver',gilt_roof=False):
    """Six-sided, chamfered architectural ridge with a narrow crest."""
    vertices=[]
    for y,w,low,edge,roof in sections:
        for x,z in [(-w*.22,roof),(w*.22,roof),(w,edge),(w*.72,low),(-w*.72,low),(-w,edge)]:
            vertices.append(m.point(center+x,y,z))
    faces=[tuple(reversed(range(6)))]
    faces += [(k*6+i,k*6+(i+1)%6,(k+1)*6+(i+1)%6,(k+1)*6+i)
              for k in range(len(sections)-1) for i in range(6)]
    faces.append(tuple(range(len(vertices)-6,len(vertices))))
    def mapping(p,n,fi):
        return m.face_uv(p,n,'brass' if gilt_roof and n[2]>.93 else style)
    m.mesh(name,vertices,faces,style,mapping=mapping)


def mass_driver_duct(m,center):
    """Closed wall volume around one truly open, chamfered rectangular bore."""
    def loop(w,h):
        return [(-w*.72,h),(w*.72,h),(w,h*.58),(w,-h*.58),
                (w*.72,-h),(-w*.72,-h),(-w,-h*.58),(-w,h*.58)]
    sections=[(0,.82,.58,.46,.30),(3.7,.73,.54,.43,.28),
              (6.3,.84,.58,.43,.28),(6.7,1.04,.64,.48,.32)]
    vertices=[]
    for y,w,h,iw,ih in sections:
        vertices += [center+np.array([x,y,z]) for x,z in loop(w,h)+loop(iw,ih)]
    faces=[]
    for k in range(len(sections)-1):
        a=k*16;b=(k+1)*16
        for i in range(8):
            j=(i+1)%8
            faces.extend([(a+i,b+i,b+j,a+j),(a+8+i,a+8+j,b+8+j,b+8+i)])
    for k in (0,len(sections)-1):
        a=k*16
        for i in range(8):
            j=(i+1)%8
            f=(a+i,a+j,a+8+j,a+8+i)
            faces.append(f if k else tuple(reversed(f)))
    def mapping(p,n,fi):
        return m.face_uv(p,n,'silver' if n[1]>.82 else 'dark')
    m.mesh('mass-driver.barrel',vertices,faces,'dark',mapping=mapping)


def build(m):
    p=m.point
    # A deep keel, low shoulders, and tiered armor give the hull architectural
    # weight. The bow is faceted; it is not a flat extruded silhouette.
    loft(m,'hull.primary-keel',[(212,8,-.08,.28,.40),(270,27,-.26,.60,.93),
        (336,43,-.61,.85,1.23),(391,43,-.89,.93,1.40),(517,44,-1.05,1.00,1.48),
        (607,47,-.98,1.04,1.54),(659,70,-.84,1.00,1.51),
        (703,88,-.73,.88,1.34),(737,78,-.57,.64,1.05)])
    loft(m,'hull.dorsal-palace-deck',[(320,19,.95,1.15,1.39),(364,35,1.03,1.34,1.60),
        (564,35,1.11,1.37,1.66),(638,36,1.10,1.34,1.62),(702,33,.94,1.22,1.40)])
    # Prow plates cradle one central capital-scale mass driver.
    for side,label in ((-1,'port'),(1,'starboard')):
        mirror=lambda x:225+side*(x-225)
        outline=[(mirror(x),y) for x,y in [(235,191),(252,228),(275,286),(306,337),
                                         (294,369),(265,355),(244,303),(235,251)]]
        m.slab('prow.'+label+'.ceramic-plate',outline,.42,1.37,.12)
        ridge=[(mirror(239),207,1.42),(mirror(252),262,1.48),(mirror(272),312,1.48),(mirror(291),344,1.39)]
        for i,(a,b) in enumerate(zip(ridge,ridge[1:])):
            m.beam('prow.'+label+f'.gilt-edge.{i}',p(*a),p(*b),.055,.075,'brass')
        m.slab('prow.'+label+'.navy-recess',[(mirror(x),y) for x,y in
            [(274,285),(292,307),(306,334),(300,344),(288,325)]],.68,.98,.04,'dark')
        m.box('hull.'+label+'.forward-shoulder',(min(mirror(268),mirror(285)),365,
              max(mirror(268),mirror(285)),411),.35,1.20,bevel=.085)
        m.box('hull.'+label+'.aft-shoulder',(min(mirror(269),mirror(298)),580,
              max(mirror(269),mirror(298)),659),.27,1.27,bevel=.1)
        for j,y in enumerate((449,521,589)):
            m.box('hull.'+label+f'.flank-plating.{j}',(min(mirror(264),mirror(281)),y,
                  max(mirror(264),mirror(281)),y+30),-.19,.75,bevel=.07)
        for j,y in enumerate((377,471,566,662)):
            # Deliberate buttress rhythm, not extra weapon mounts.
            x=mirror(259)
            m.beam('hull.'+label+f'.buttress.{j}',p(x,y,-.51),p(mirror(249),y,1.25),.14,.20,'silver')
        panel_band(m,'hull.'+label+'.belt-trim',mirror(267),379,mirror(271),627,.83,.045)
        m.slab('hull.'+label+'.broadside-gallery',[(mirror(x),y) for x,y in
            [(275,373),(296,403),(303,458),(302,568),(292,609),(275,638),(267,602),(266,408)]],
            -.38,.85,.085,'dark')
        panel_band(m,'hull.'+label+'.gallery-cornice',mirror(282),408,mirror(282),595,.88,.047)

    # Three staggered ridges retain the Authority seal. Sculpted ceramic
    # fairings sit over recessed navy cores, with actual joints and buttresses.
    for i,(x,y_start,roof) in enumerate(((204,330,2.19),(225,284,2.39),(246,243,2.58))):
        core=[(y_start,1.25,1.18,1.30,1.42),(y_start+42,6.8,1.21,roof-.38,roof-.18),
              (592,6.8,1.21,roof-.30,roof-.08),(682,5.3,1.08,roof-.47,roof-.29),
              (710,2.7,1.00,1.26,1.50)]
        fairing(m,f'spine.{i+1}.ridge',core,center=x,style='dark')
        height=lambda y:float(np.interp(y,[s[0] for s in core],[s[4] for s in core]))+.20
        width=lambda y:float(np.interp(y,[s[0] for s in core],[s[1] for s in core]))+.5
        stations=[y_start,420,510,602,710]
        for j,(start,end) in enumerate(zip(stations,stations[1:])):
            a=start+(2 if j==0 else 4);b=end-4
            mid=a+min(28,(b-a)*.24)
            sections=[]
            for y,w,tip in ((a,width(a)*.70,.08),(mid,width(mid),0),(b,width(b)*.83,.035)):
                z=height(y)-tip
                sections.append((y,w,z-.62,z-.30,z))
            fairing(m,f'spine.{i+1}.armor-segment.{j+1}',sections,center=x,gilt_roof=True)
        for j,y in enumerate((y_start+80,509,596)):
            z=height(y)
            fairing(m,f'spine.{i+1}.root-buttress.{j+1}',[(y-2,8.8,1.12,z-.46,z-.25),
                (y+2,8.8,1.12,z-.46,z-.25)],center=x)
        for side in (-1,1):
            m.beam(f'spine.{i+1}.status-slit.{side}',p(x+side*6.65,480,height(485)-.37),
                p(x+side*6.65,488,height(485)-.37),.024,.032,'cyan')

    # One very large kinetic weapon. A deep rectangular launch bore, two
    # armored acceleration rails, a substantial breech and recoil sleeves give
    # it the mass of a siege gun. There is no beam-emitter lens in the muzzle.
    driver_center=p(225,258,.64)
    mass_driver_duct(m,driver_center)
    loft(m,'mass-driver.armored-breech',[(307,24,-.02,.99,1.45),(284,30,-.08,1.12,1.55),
        (260,25,-.04,1.06,1.42),(247,20,.06,.99,1.30)],style='silver')
    m.box('mass-driver.breech-chamber',(210,229,240,236),.08,1.20,'dark',.035,3)
    fairing(m,'mass-driver.feed-keel',[(303,14,-.35,-.03,.19),(260,11,-.29,.08,.22)],style='dark')
    for side,label in ((-1,'port'),(1,'starboard')):
        fairing(m,f'mass-driver.rail-casing.{label}',[(251,5.5,.22,1.07,1.34),(200,8,.18,1.10,1.39),
            (134,7,.17,1.02,1.22),(102,7.5,.11,1.06,1.25)],center=225+side*21,gilt_roof=True)
        m.beam(f'mass-driver.rail.{label}',driver_center+[side*.431,.12,0],
            driver_center+[side*.431,6.42,0],.05,.10,'silver')
        m.cylinder(f'mass-driver.recoil-sleeve.{label}',p(225+side*24,296,.65),
            p(225+side*24,239,.65),.19,'dark',8)
        for j,y in enumerate((218,161)):
            m.beam(f'mass-driver.status.{label}.{j+1}',p(225+side*13,y,1.205),
                p(225+side*13,y-8,1.205),.035,.03,'cyan')
    for j,y in enumerate((220,180,145,112)):
        m.beam(f'mass-driver.upper-clamp.{j+1}',p(208,y,1.25),p(242,y,1.25),.22,.14,'silver')
        m.beam(f'mass-driver.lower-clamp.{j+1}',p(208,y,.055),p(242,y,.055),.10,.085,'dark')
    for z,label in ((.64+.64,'upper'),(.64-.64,'lower')):
        m.beam(f'mass-driver.muzzle-rim.{label}',driver_center+[-.73,6.705,z-.64],
            driver_center+[.73,6.705,z-.64],.065,.06,'brass')
    m.socket('socket.mass-driver.muzzle',driver_center+[0,6.80,0],[0,1,0],'capital-mass-driver')

    # Four slim structural struts support one continuous, horizontal halo.
    for i,degrees in enumerate((62,118,242,298)):
        angle=math.radians(degrees)
        axis=np.array([math.cos(angle),math.sin(angle),0])
        m.beam(f'halo.strut.{i+1}',axis*2.0+[0,0,.69],axis*6.78+[0,0,.69],.20,.19,'silver')
        m.beam(f'halo.strut.{i+1}.gold-inlay',axis*2.2+[0,0,.79],axis*6.76+[0,0,.79],.036,.028,'brass')
    m.assembly('interdiction-halo',role='interdiction-projector')
    halo_sector(m,'halo.continuous-armored-ring',[(6.48,.60),(6.73,.57),(6.85,.66),
        (6.85,.79),(6.76,.87),(6.49,.87),(6.43,.80),(6.43,.68)])
    halo_sector(m,'halo.outer-gold-binding',[(6.80,.81),(6.85,.77),(6.86,.82),(6.81,.89)],style='brass')
    halo_sector(m,'halo.inner-gold-binding',[(6.425,.69),(6.475,.69),(6.475,.90),(6.425,.90)],style='brass')
    for i in range(12):
        angle=i*math.tau/12
        halo_sector(m,f'halo.panel-joint.{i}',[(6.43,.873),(6.85,.873),(6.85,.905),(6.43,.905)],
                    angle-.006,.012,1,'dark')
    for i in range(8):
        angle=(i+.5)*math.tau/8
        center=np.array([math.cos(angle),math.sin(angle),0])*6.65
        tangent=np.array([-math.sin(angle),math.cos(angle),0])
        m.beam(f'halo.emitter.{i}.housing',center-tangent*.24+[0,0,.92],center+tangent*.24+[0,0,.92],.32,.14,'dark')
        m.beam(f'halo.emitter.{i}.lens',center-tangent*.145+[0,0,1.006],center+tangent*.145+[0,0,1.006],.15,.035,'cyan')
    m.socket('socket.interdiction.center',[0,0,.74],[0,0,1],'interdiction-field')
    m.assembly('hull')

    # Command citadel: a single narrow tower with successive setbacks and gold
    # cornices. Warm clerestory glazing distinguishes it from the player fleet.
    tower_sections=[(608,13,1.18,1.74,2.08),(623,24,1.20,1.93,2.40),
                    (664,24,1.20,1.93,2.40),(687,16,1.10,1.61,1.90)]
    loft(m,'command.tower-foundation',tower_sections)
    m.box('command.lower-tier',(207,624,244,670),2.1,2.80,bevel=.13,corner=6)
    m.box('command.first-cornice',(205,622,246,672),2.69,2.85,'brass',.045,5)
    m.box('command.narrow-tower',(211,628,240,665),2.82,3.98,'silver',.11,4)
    m.box('command.upper-cornice',(208,625,243,668),3.93,4.09,'brass',.045,5)
    m.box('command.amber-bridge',(211,629,240,664),4.07,4.43,'amber',.055,4)
    m.box('command.bridge-roof',(208,626,243,667),4.42,4.57,'silver',.06,5)
    m.box('command.roof-gold-edge',(208,626,243,667),4.405,4.45,'brass',.013,5)
    loft(m,'command.crown',[(632,6,4.53,4.67,4.90),(649,9,4.53,4.77,5.12),(660,5,4.53,4.67,4.82)],style='silver')
    m.lathe('command.single-crown-finial',p(225,648,4.94),[0,0,1],[(0,.10),(.20,.08),(.60,.018)],12,'brass','brass')
    for side in (-1,1):
        for j,y in enumerate((630,654)):
            x=225+side*18
            m.beam(f'command.buttress.{side}.{j}',p(x+side*5,y,1.70),p(x,y,3.93),.135,.23,'silver')
            m.beam(f'command.buttress-gold.{side}.{j}',p(x+side*5.3,y,1.73),p(x+side*.9,y,3.90),.029,.035,'brass')
        # Window mullions split the continuous amber band into narrow lights.
        x=211 if side<0 else 240
        for j,y in enumerate((632,641,650,659)):
            m.beam(f'command.window-mullion.{side}.{j}',p(x,y,4.09),p(x,y,4.44),.036,.04,'dark')
    m.socket('socket.command',p(225,645,4.28),[0,1,0],'command-tower')

    # One closed ventral hangar. Door panels sit within a dark armored coaming,
    # leaving a central seam and four deep framed service bays aft of it.
    m.box('hangar.outer-coaming',(197,356,254,628),-1.18,-.78,bevel=.1,corner=6)
    m.box('hangar.recess',(201,364,250,620),-1.24,-1.14,'dark',.028,3)
    for side,(x0,x1) in enumerate(((203,224),(226,248))):
        m.box(f'hangar.closed-door.{side+1}',(x0,368,x1,614),-1.29,-1.20,bevel=.038,corner=2)
    for i,y in enumerate((379,427,475,523,571,606)):
        m.beam(f'hangar.door-stiffener.{i}',p(205,y,-1.303),p(246,y,-1.303),.07,.055,'dark')
    for side in (-1,1):
        x=225+side*33
        m.beam(f'ventral.keel-rail.{side}',p(x,371,-.97),p(x,646,-.91),.14,.18,'dark')
        for i,y in enumerate((372,435,500,567,646)):
            m.box(f'ventral.maintenance.{side}.{i}',(x-8,y,x+8,y+28),-.96,-.73,bevel=.045)
    m.box('ventral.aft-keel',(198,649,253,707),-1.01,-.63,bevel=.11)

    # Four equal engines in one straight horizontal bank, with ivory cowls and
    # recessed cyan exhausts. No exhaust discs are projected onto their tops.
    for i,x in enumerate((146,199,252,305)):
        name=f'engine.{i+1}'
        center=p(x,727,.04)
        m.cylinder(name+'.pylon',p(x,693,.12),center,.60,'dark',16)
        m.lathe(name+'.cylindrical-housing',center,[0,-1,0],[(0,.72),(.16,.85),(.44,.93),
              (1.18,.93),(1.42,.86),(1.72,.83),(2.06,.86),(2.19,.81)],20,'auto','enginewall')
        m.lathe(name+'.nozzle-rim',center,[0,-1,0],[(1.98,.875),(2.27,.875),(2.31,.69),
                    (2.04,.61)],24,'dark','enginewall',ring=True)
        m.lathe(name+'.gilt-nozzle-binding',center,[0,-1,0],[(1.65,.864),(1.76,.864),
                    (1.76,.819),(1.65,.819)],16,'brass','brass',ring=True)
        m.lathe(name+'.exhaust-disc',center,[0,-1,0],[(2.245,.670),(2.26,.670)],16,'dark','dark',cap=(1197,516,34))
        loft(m,name+'.white-cowl',[(665,20,-.20,.94,1.43),(681,26,-.28,1.24,1.79),
              (720,24,-.20,1.24,1.76),(739,20,-.10,.99,1.35)],center=x)
        for side in (-1,1):
            m.beam(name+f'.cowl-binding.{side}',p(x+side*20,683,1.45),p(x+side*19,729,1.50),.045,.045,'brass')
        for j in range(8):
            a=j*math.tau/8
            q=np.array([math.cos(a)*.885,0,math.sin(a)*.885])
            m.beam(name+f'.cooling-rib.{j}',center+q+[0,-.55,0],center+q+[0,-1.40,0],.035,.045,'dark')
        m.socket(f'socket.engine.{i+1}',center+[0,-2.34,0],[0,-1,0],'engine-exhaust')

    # Six turrets: mounts remain part of the static hull, gun assemblies use
    # six proper local yaw pivots. Each turret has exactly two modeled barrels.
    for side,label in ((-1,'port'),(1,'starboard')):
        for j,y in enumerate((423,493,559)):
            x=225+side*80
            x0,x1=sorted((x-side*9,x+side*27))
            m.beam(f'mount.{label}.{j+1}.armored-spar',p(225+side*33,y,.60),p(x+side*7,y,.60),.80,.76,'dark')
            m.slab(f'mount.{label}.{j+1}',[(x0,y-23),(x1,y-18),(x1,y+18),(x0,y+23)],.36,1.29,.10)
            pivot=p(x,y,1.31)
            m.lathe(f'mount.{label}.{j+1}.barbette',pivot,[0,0,1],[(0,.60),(.10,.69),(.29,.69),(.37,.55)],16,'silver','silver')
            assembly=f'turret-{label}-{j+1}'
            m.assembly(assembly,pivot,'turret-yaw')
            m.box(assembly+'.armored-housing',(x-17,y-18,x+17,y+18),1.55,2.19,'silver',.11,6)
            m.box(assembly+'.gold-base',(x-18,y-19,x+18,y+19),1.51,1.63,'brass',.035,5)
            m.box(assembly+'.roof-plate',(x-13,y-14,x+13,y+14),2.14,2.27,'silver',bevel=.06,corner=5)
            m.box(assembly+'.sighting-recess',(x-4,y-7,x+4,y+7),2.26,2.28,'dark',bevel=.006,corner=2)
            m.cylinder(assembly+'.trunnion',p(x+side*11,y-14,1.88),p(x+side*11,y+14,1.88),.16,'dark',12)
            for barrel,offset in enumerate((-.21,.21)):
                base=p(x+side*11,y,1.88)+[0,offset,0]
                axis=[side,0,0]
                m.lathe(assembly+f'.barrel.{barrel+1}',base,axis,[(0,.15),(.19,.17),(.40,.14),
                    (.65,.115),(1.21,.10),(1.49,.105)],12,'dark','enginewall')
                m.lathe(assembly+f'.muzzle-rim.{barrel+1}',base,axis,[(1.44,.126),(1.57,.126),
                    (1.57,.074),(1.44,.074)],8,'silver','silver',ring=True)
                m.socket(f'socket.muzzle.{label}.{j+1}.{barrel+1}',base+np.array(axis)*1.60,axis,'turret-muzzle',assembly=assembly)
            m.socket(f'socket.turret.{label}.{j+1}',pivot,[side,0,0],'heavy-twin-turret',assembly=assembly,barrels=2)
            m.assembly('hull')

    # Additional armament requested after the reference was approved. Four
    # paired medium batteries per broadside sit fore/aft of the main gun rows.
    # Their mounts clear the halo and preserve its thin, uninterrupted outline.
    for side,label in ((-1,'port'),(1,'starboard')):
        for j,(offset,y,z,forward) in enumerate(((62,321,1.28,.46),(49,389,1.23,.18),
                                                (55,614,1.32,-.20),(86,684,1.80,-.38))):
            x=225+side*offset
            pivot=p(x,y,z)
            m.lathe(f'medium-mount.{label}.{j+1}',pivot,[0,0,1],[(-.16,.35),(.10,.41),(.18,.35)],12,'dark','dark')
            assembly=f'secondary-{label}-{j+1}'
            m.assembly(assembly,pivot,'secondary-turret-yaw')
            m.box(assembly+'.armored-housing',(x-10,y-11,x+10,y+11),z+.11,z+.53,'silver',.085,4)
            m.beam(assembly+'.gold-cornice',p(x-side*5,y-9,z+.51),p(x-side*5,y+9,z+.51),.055,.055,'brass')
            axis=unit([side,forward,0]);across=unit(np.cross(axis,[0,0,1]))
            for barrel in range(2):
                base=p(x+side*5,y,z+.34)+across*((barrel-.5)*.23)
                m.lathe(assembly+f'.barrel.{barrel+1}',base,axis,[(0,.095),(.16,.107),(.32,.075),(.98,.065)],8,'dark','dark')
                m.lathe(assembly+f'.muzzle.{barrel+1}',base,axis,[(.91,.077),(1.04,.077),(1.04,.047),(.91,.047)],8,'brass','brass',ring=True)
                m.socket(f'socket.secondary.{label}.{j+1}.{barrel+1}',base+axis*1.055,axis,'secondary-muzzle',assembly=assembly)
            m.socket(f'socket.secondary.{label}.{j+1}',pivot,axis,'secondary-twin-turret',assembly=assembly,barrels=2)
            m.assembly('hull')

    # Eight low-profile paired defense mounts: four dorsal and four ventral.
    # The ventral stations remain clear of the boarding hangar's closed doors.
    for bottom in (False,True):
        surface='ventral' if bottom else 'dorsal'
        for side,label in ((-1,'port'),(1,'starboard')):
            for j,y in enumerate((458,588)):
                x=225+side*(36 if bottom else 32)
                z=-1.09 if bottom else 1.53
                pivot=p(x,y,z)
                sign=-1 if bottom else 1
                axis=unit([side,.48 if j==0 else -.48,sign*.08])
                assembly=f'point-defense-{surface}-{label}-{j+1}'
                m.cylinder(assembly+'.fixed-pedestal',pivot+[0,0,-sign*.22],pivot+[0,0,sign*.08],.18,'dark',6)
                m.assembly(assembly,pivot,'point-defense-yaw')
                low,high=sorted((z,z+sign*.28))
                m.box(assembly+'.armored-housing',(x-6,y-8,x+6,y+8),low,high,'silver',.035,3)
                across=unit(np.cross(axis,[0,0,1]))
                for barrel in range(2):
                    base=p(x+side*4,y,z+sign*.15)+across*((barrel-.5)*.135)
                    m.lathe(assembly+f'.barrel.{barrel+1}',base,axis,[(0,.047),(.16,.055),(.55,.038)],6,'dark','dark')
                    m.socket(f'socket.point-defense.{surface}.{label}.{j+1}.{barrel+1}',base+axis*.56,axis,'point-defense-muzzle',assembly=assembly)
                m.socket(f'socket.point-defense.{surface}.{label}.{j+1}',pivot,axis,'paired-point-defense',assembly=assembly,barrels=2)
                m.assembly('hull')


if __name__=='__main__':
    model=Reconstruction(ROOT,SPEC)
    model.prepare_textures();build(model);model.export()
