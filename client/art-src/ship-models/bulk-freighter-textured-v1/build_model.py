"""Bulk Freighter: sixteen cargo modules on an open lattice spine.

Direct Python/NumPy meshes, five-view UV projections, and eight coplanar engines.
TOP/BOTTOM determine stations; elevation artwork supplies vertical surfaces.
"""
from geometry import *


def belly_mapping(n,c):
    if abs(n[2])>.28:return ventral,bottom_uv
    return graphite,top_uv

def service_mapping(n,c):
    if n[2]<-.28:return ventral,bottom_uv
    if n[2]>.28:return atlas,top_uv
    return graphite,top_uv

def gold_mapping(n,c):
    if n[2]<-.28:return ventral,bottom_uv
    if n[2]>.28:return atlas,top_uv
    return brass,top_uv

def silo_mapping(bounds,row):
    x0,y0,x1,y1=bounds
    u0,u1=[(1018,1144),(1187,1299),(1340,1449),(1489,1610)][row-1]
    def mapping(n,c):
        if n[2]<-.30:return ventral,bottom_uv
        if n[2]>.30:return atlas,top_uv
        if abs(n[0])>.60:
            def uv(v):
                py=CY-v[1]*S
                return ((u0+(py-y0)/(y1-y0)*(u1-u0))/2048,
                        (797-v[2]*48)/2048)
            return atlas,uv
        # The matching forward side of a barrel is also valid on the aft side;
        # using BACK globally would paint exhausts onto the mineral silos.
        return atlas,front_uv
    return mapping

def cage_mapping(bounds):
    x0,y0,x1,y1=bounds
    def mapping(n,c):
        if n[2]<-.30:return ventral,bottom_uv
        if n[2]>.30:return atlas,top_uv
        if abs(n[0])>.55:
            # A complete X-paneled cage in SIDE supplies all four stations;
            # two cage end faces are obscured by trusses in the illustration.
            def uv(v):
                py=CY-v[1]*S
                return ((1300+(py-y0)/(y1-y0)*38)/2048,
                        (772-(v[2]-.50)/1.06*45)/2048)
            return atlas,uv
        # Reuse the cage panel in TOP on its hidden fore/aft vertical faces.
        def uv(v):
            px=CX+v[0]*S
            return ((x0+6+(px-x0)/(x1-x0)*(x1-x0-12))/2048,
                    (y1-5-(v[2]-.50)/1.06*(y1-y0-10))/2048)
        return atlas,uv
    return mapping

def bridge_mapping(n,c):
    py=CY-c[1]*S
    if n[2]<-.30:return ventral,bottom_uv
    if n[1]>.25 and py<97:
        def uv(v):return ((1322-v[0]*79)/2048,(295-v[2]*36)/2048)
        return atlas,uv
    if abs(n[0])>.55:
        def uv(v):
            py=CY-v[1]*S
            return ((interpolate(py,[(63,916),(85,941),(106,963),(145,1000),(171,1024)]))/2048,
                    (776-v[2]*33)/2048)
        return atlas,uv
    if n[2]>.25:
        def uv(v):
            py=CY-v[1]*S
            py=interpolate(py,[(63,63),(81,81),(97,105),(118,125),(155,155),(171,171)])
            return ((CX+v[0]*S)/2048,py/2048)
        return atlas,uv
    return graphite,top_uv

def sensor_mapping(n,c):
    if n[2]<-.30:return ventral,bottom_uv
    if n[2]>.30:return atlas,top_uv
    if abs(n[0])>.5:
        def uv(v):return ((250+(v[2]-.67)/1.25*11)/2048,(CY-v[1]*S)/2048)
        return atlas,uv
    return graphite,top_uv

def reactor_mapping(n,c):
    if n[2]<-.30:return ventral,bottom_uv
    if n[2]>.30:return atlas,top_uv
    if n[1]<-.45:
        def uv(v):return ((1321+v[0]*72)/2048,(581-v[2]*68)/2048)
        return atlas,uv
    if abs(n[0])>.55:
        def uv(v):
            py=CY-v[1]*S
            return ((1653+(py-696)/98*57)/2048,(804-v[2]*40)/2048)
        return atlas,uv
    return graphite,top_uv

def drive_mapping(n,c):
    if n[2]<-.30:return ventral,bottom_uv
    if n[2]>.30:return atlas,top_uv
    if abs(n[0])>.55:return atlas,side_uv
    # Solid drive walls use the unoccluded top panels, not painted exhausts.
    def uv(v):return ((CX+v[0]*S)/2048,(786-(v[2]-.24)/1.65*67)/2048)
    return atlas,uv


def box_truss(name,x0,x1,y0,y1,low,high,width=.045):
    rings=[[point(x0,y0,z),point(x1,y0,z),point(x1,y1,z),point(x0,y1,z)]
           for z in (low,high)]
    edges=[]
    for r in rings:
        edges.extend((r[i],r[(i+1)%4]) for i in range(4))
        edges.extend([(r[0],r[2]),(r[1],r[3])])
    edges.extend((rings[0][i],rings[1][i]) for i in range(4))
    for i,j in ((0,1),(3,2)):
        edges.extend([(rings[0][i],rings[1][j]),(rings[1][i],rings[0][j])])
    for i,(a,b) in enumerate(edges,1):
        beam(name+f' | brace {i}',a,b,width,width*1.3,silver if i%3 else graphite)


# A true four-chord lattice spine, open between rails and diagonal members.
for side,label,x in [(-1,'port',218),(1,'starboard',236)]:
    for z,level in [(.49,'lower'),(1.61,'upper')]:
        beam(f'structure.spine.{label} | {level} chord',point(x,144,z),
             point(x,724,z),.075,.09,graphite)
    beam(f'structure.spine.{label} | machined dorsal edge',point(x,153,1.66),
         point(x,704,1.66),.035,.045,silver)
stations=[148,224,281,367,421,508,562,649,700,721]
for i,py in enumerate(stations,1):
    for z,level in [(.49,'lower'),(1.61,'upper')]:
        beam(f'structure.spine | {level} cross tie {i}',point(218,py,z),
             point(236,py,z),.055,.08,silver)
    for x in (218,236):
        beam(f'structure.spine | vertical tie {i}.{x}',point(x,py,.49),
             point(x,py,1.61),.06,.06,graphite)
for i,(a,b) in enumerate(zip(stations,stations[1:]),1):
    for z,level in [(.50,'lower'),(1.60,'upper')]:
        for j,(x0,x1) in enumerate([(219,235),(235,219)],1):
            beam(f'structure.spine | {level} diagonal {i}.{j}',point(x0,a+2,z),
                 point(x1,b-2,z),.042,.050,silver)
    for x in (218,236):
        beam(f'structure.spine | flank diagonal {i}.{x}',point(x,a,.52),
             point(x,b,1.57),.038,.05,graphite)

for row,(a,b) in enumerate([(212,270),(354,410),(494,552),(638,689)],1):
    bevel_box(f'spine.{row} | dorsal distribution housing',(214,a,240,b),1.48,1.88,
              bevel=.05,corner=.12,mode=service_mapping)
    bevel_box(f'spine.{row} | ventral service housing',(216,a+2,238,b-2),.30,.58,
              bevel=.045,corner=.08,mode=belly_mapping)
    for py in (a+4,b-5):
        bevel_box(f'lock.spine.{row} | amber collar {py}',(216,py-2,238,py+2),
                  1.78,1.93,bevel=.025,corner=.04,mode=gold_mapping)

# Eight cargo stations: a large transverse silo followed by a smaller cage,
# repeated four times. Port/starboard are physically separate closed modules.
silo_rows=[(151,228),(291,368),(432,509),(573,650)]
cage_rows=[(233,277),(374,418),(515,559),(656,700)]
for side,label,x0,x1,cx in [(-1,'port',32,186,115),(1,'starboard',268,422,339)]:
    for row,((y0,y1),(c0,c1)) in enumerate(zip(silo_rows,cage_rows),1):
        prefix=f'cargo.{label}.{row}'
        cy=(y0+y1)/2;ry=(y1-y0)/S/2
        transverse_shell(prefix+' | mineral silo',
            [(x0,.57),(x0+5,.79),(x0+17,1),(x1-17,1),(x1-5,.79),(x1,.57)],
            cy,ry,1.00,1.10,mode=silo_mapping((x0,y0,x1,y1),row),segments=32)
        # A narrow band follows the same rounded cross section, with a solid
        # thickness instead of a flat strip hovering over the curved shell.
        transverse_shell(f'restraint.{label}.{row} | silo belly band',
            [(cx-5,1.016),(cx+5,1.016),(cx+5,1.045),(cx-5,1.045),(cx-5,1.016)],
            cy,ry,1.00,1.10,mat=graphite,cap=False,segments=32)
        for py,which in [(y0+5,'forward'),(y1-5,'aft')]:
            # Fit the lock to the curved strap; a horizontal box at roof
            # height would float above this shoulder of the rounded shell.
            def band_surface(q):
                return 1.10+(1.045**3-abs((q-cy)/(ry*S))**3)**(1/3)
            beam(f'lock.{label}.{row} | silo {which} buckle',
                 point(cx,py-3,band_surface(py-3)+.018),
                 point(cx,py+3,band_surface(py+3)+.018),.18,.085,brass)
        for hx in (x0+31,x1-31):
            bevel_box(prefix+f' | silo sump hatch {hx}',(hx-6,cy-7,hx+6,cy+7),
                      .052,.14,bevel=.024,corner=.05,mode=belly_mapping)
        sx0,sx1=(47,182) if side<0 else (272,407)
        bounds=(sx0,c0,sx1,c1)
        bevel_box(prefix+' | rectangular cargo cage',bounds,.48,1.57,
                  bevel=.065,corner=.19,mode=cage_mapping(bounds))
        bevel_box(prefix+' | cage ventral inspection panel',
                  (sx0+9,c0+5,sx1-9,c1-5),.405,.535,
                  bevel=.03,corner=.08,mode=belly_mapping)
        for z,level in [(1.60,'dorsal'),(.43,'ventral')]:
            bevel_box(f'restraint.{label}.{row} | cage {level} band',
                      (cx-4.5,c0+1,cx+4.5,c1-1),z-.045,z+.045,
                      bevel=.018,corner=.04,mode=service_mapping if z>1 else belly_mapping)
        for py in (c0+3,c1-3):
            bevel_box(f'lock.{label}.{row} | cage buckle {py}',(cx-3,py-2,cx+3,py+2),
                      1.60,1.73,bevel=.018,corner=.04,mode=gold_mapping)
        for kind,py,span,lo,hi,edge in [('silo',cy,14,.67,1.53,x1 if side<0 else x0),
                                       ('cage',(c0+c1)/2,10,.66,1.40,sx1 if side<0 else sx0)]:
            spine=218 if side<0 else 236
            a,b=sorted((edge-2*side,spine))
            box_truss(f'structure.cargo.{label}.{row}.{kind}',a,b,py-span,py+span,lo,hi)
            bevel_box(f'anchor.{label}.{row}.{kind} | gold docking shoe',
                      (edge-2,py-span,edge+2,py+span),lo-.025,hi+.025,
                      bevel=.025,corner=.02,mat=brass)
            sockets.append((f'socket.cargo.{label}.{row}.{kind}',point(cx,py,1.10)))
        # Small outward-facing position strips, inset in dark housings.
        outer=x0 if side<0 else x1
        bevel_box(f'light.{label}.{row} | lamp housing',(outer-.65,cy-3,outer+.65,cy+3),
                  .99,1.23,bevel=.012,corner=.01,mat=dark)
        beam(f'light.{label}.{row} | cyan running strip',point(outer+side*.72,cy,1.03),
             point(outer+side*.72,cy,1.19),.018,.024,cyan)

# Tiny command pod at the forward end, with dedicated bottom projection.
loft('bridge | faceted command pod',[
    (63,6,.88,1.30,1.43),(75,10,.76,1.51,1.69),(88,14,.65,1.69,1.91),
    (99,17,.61,1.78,2.00),(117,21,.55,1.65,1.95),
    (142,25,.53,1.57,1.89),(156,18,.63,1.43,1.73),(171,10,.84,1.41,1.53),
],mode=bridge_mapping)
for side,label in [(-1,'port'),(1,'starboard')]:
    slab(f'bridge.{label} | sensor fairing',[(248,109),(255,119),(260,137),(254,151),(249,145)],
         1.65,.67,side=side,bevel=.045,mode=sensor_mapping)
bevel_box('bridge | ventral access hatch',(220,111,234,145),.46,.59,
          bevel=.025,corner=.06,mode=belly_mapping)
radial('sensor | axial probe',CX,[(47,.026),(59,.035),(68,.069)],1.18,
       mat=graphite,segments=24)
ring('sensor | amber probe collar',CX,62,1.18,.076,.043,.065,brass)
sockets.append(('socket.bridge',point(CX,100,2.015)))

# Wide rear reactor/drive pack at the final lattice station. It is narrower
# than the mineral silos and contains one coplanar row of eight engines.
drive_outline=[(165,704),(289,704),(330,730),(354,753),(354,785),
               (335,799),(119,799),(100,784),(100,754),(125,729)]
slab('drive | eight-engine support',drive_outline,1.90,.24,bevel=.10,mode=drive_mapping)
loft('stern | central reactor-service fairing',[
    (662,9,.55,1.58,1.86),(686,18,.36,1.89,2.26),(708,28,.20,2.08,2.54),
    (745,24,.20,1.99,2.48),(777,18,.26,1.88,2.33),(797,14,.41,1.72,1.96),
],mode=reactor_mapping)
for side,label in [(-1,'port'),(1,'starboard')]:
    for i,(x,a,b) in enumerate([(153,733,783),(178,718,782),(200,715,792)],1):
        px=CX+side*(CX-x)
        bevel_box(f'drive.{label}.{i} | radiator housing',(px-7,a,px+7,b),1.79,2.02,
                  bevel=.035,corner=.08,mode=service_mapping)
        if i<3:
            radial(f'fitting.drive.{label}.{i} | coolant conduit',px,
                   [(a-5,.057),(b+4,.057)],2.03,mat=brass,segments=24)
beam('structure.drive | aft engine crossbeam',point(119,785,1.0),
     point(335,785,1.0),.16,.23,graphite)
sockets.append(('socket.reactor',point(CX,713,2.56)))

engine_positions=[132,159,186,213,241,268,295,322]
rear_pixels=[1118,1175,1232,1290,1349,1408,1467,1526]
for i,(px,ref_x) in enumerate(zip(engine_positions,rear_pixels),1):
    prefix=f'engine.{i}';z=1.00
    def casing_mapping(n,c):
        if n[2]<-.28:return ventral,bottom_uv
        if n[2]>.28:return atlas,top_uv
        if abs(n[1])>.75:return graphite,top_uv
        def uv(v):
            py=CY-v[1]*S
            return ((1696+(py-770)*.85)/2048,(772-v[2]*24)/2048)
        return atlas,uv
    radial(prefix+' | armored engine casing',px,
           [(766,.25),(775,.325),(788,.33),(801,.305)],z,
           mode=casing_mapping,segments=40)
    ring(prefix+' | graphite forward coupling',px,773,z,.318,.261,.065,graphite)
    ring(prefix+' | amber thermal collar',px,799,z,.32,.26,.065,brass)
    ring(prefix+' | machined exhaust shoulder',px,802,z,.311,.249,.070,silver)
    radial(prefix+' | tapered nozzle jacket',px,
           [(801,.309),(811,.296),(823,.279),(827,.279)],z,
           mat=dark,cap=False,segments=40)
    ring(prefix+' | outer nozzle lip',px,826,z,.281,.228,.080,dark)
    ring(prefix+' | inner recessed throat',px,820,z,.232,.188,.14,silver)
    ring(prefix+' | aperture retaining collar',px,816,z,.196,.176,.040,brass)
    def exhaust_mapping(n,c,px=px,ref_x=ref_x,z=z):
        if abs(n[1])<.9:return dark,top_uv
        def uv(v):
            return ((ref_x+(CX+v[0]*S-px)/S*15/.18)/2048,
                    (559-(v[2]-z)*15/.18)/2048)
        return atlas,uv
    radial(prefix+' | recessed textured ion aperture',px,
           [(815,.18),(815.5,.18)],z,mode=exhaust_mapping,segments=40)
    sockets.append((f'socket.engine.{i}',point(px,829,z)))

export()
