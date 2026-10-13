"""Reconstruct the approved Tiny Freighter with solid geometry and four-view UVs.

Y-up/-Z-forward GLB and editable OBJ are exported without a DCC application.
The top view controls planform; component-local side projections reconcile the
illustration's different proportions. No generated replacement texture is used.
"""
from geometry import *

def cargo_mapping(n,center):
    if n[2]<-.36:return ventral,bottom_uv
    if n[2]>.36:return atlas,top_uv
    if abs(n[0])>=abs(n[1]):return atlas,side_uv
    # End walls are divided at the rails, allowing both visible cargo shoulders
    # to use the FRONT artwork without stretching across the hidden middle.
    if abs(center[0])>1.60:return atlas,front_uv
    # Reconstruct the end panel obscured by the drawn cockpit using matching
    # cargo artwork. This avoids painting a second cabin onto the container.
    def end_panel(v):
        return ((194+(v[0]+1.72)/3.44*126)/2048,(461-v[2]/3.04*132)/2048)
    return atlas,end_panel

def cargo_box(name,y0,y1):
    vertices=[]
    for inset,z in [(11,0),(0,.22),(0,2.82),(11,3.04)]:
        x0,x1=68+inset,447-inset
        front,back=y0+inset,y1-inset
        corner=30.5-inset*.4
        # The extra collinear stations create separate end-wall surfaces at
        # each retaining rail; top and underside remain one continuous panel.
        outline=[(x0+corner,front),(171,front),(343,front),(x1-corner,front),
                 (x1,front+corner),(x1,back-corner),(x1-corner,back),
                 (343,back),(171,back),(x0+corner,back),
                 (x0,back-corner),(x0,front+corner)]
        vertices.extend(point(x,y,z) for x,y in outline)
    n=12
    faces=[tuple(reversed(range(n)))]
    for k in range(3):
        faces.extend((k*n+i,k*n+(i+1)%n,(k+1)*n+(i+1)%n,(k+1)*n+i) for i in range(n))
    faces.append(tuple(range(3*n,4*n)))
    return mesh(name,vertices,faces,mode=cargo_mapping)

def rail_mapping(n,center):
    if n[2]>.38:return atlas,top_uv
    if n[2]<-.38:return ventral,bottom_uv
    return graphite,top_uv

def bottom_mapping(n,center):
    if abs(n[2])>.28:return ventral,bottom_uv
    return graphite,top_uv

def cabin_mapping(n,center):
    if n[2]<-.32:return ventral,bottom_uv
    if n[2]>.28:return atlas,top_uv
    if abs(n[0])>abs(n[1]):return atlas,side_uv
    def uv(v):
        return ((1330-v[0]*66)/2048,(732-v[2]*65)/2048)
    return atlas,uv

# Continuous internal keel: the cabin, freight body, and aft machinery are
# connected solids, with actual depth and fully resolved undersides.
loft('hull | continuous structural keel',[
    (279,27,1.00,1.60,2.04),(304,38,.30,2.20,2.76),
    (325,65,.11,2.44,2.76),(468,67,.09,2.50,2.78),
    (495,67,.09,2.50,2.78),(638,65,.12,2.47,2.74),
    (662,49,.37,2.25,2.54),(706,34,.65,1.83,2.14),
    (730,20,.94,1.60,1.85),
],mode='auto')

# Two broad, closely coupled freight sections with chamfered corners.
for row,(y0,y1) in enumerate([(313,476),(484,650)],1):
    name=f'cargo.{row}'
    cargo_box(name+' | beveled pressure container',y0,y1)
    bevel_box(name+' | inset dorsal loading panel',(194,y0+17,320,y1-13),
              3.025,3.085,bevel=.025,corner=.075,mode=cargo_mapping)
    bevel_box(name+' | ventral access hatch',(193,y0+18,322,y1-16),
              -.085,.055,bevel=.04,corner=.09,mode=bottom_mapping)
    # Keep each outward side's light visible in both oblique and side views.
    for side,label in [(-1,'port'),(1,'starboard')]:
        x=68 if side<0 else 447
        middle=419 if row==1 else 536
        bevel_box(name+'.'+label+' | flush running-light surround',
                  (x-.8,middle-9,x+.8,middle+9),.45,.70,
                  bevel=.023,corner=.045,mat=dark)
        beam(name+'.'+label+' | cyan running strip',
             point(x+side*1.1,middle-6,.585),point(x+side*1.1,middle+6,.585),
             .024,.045,cyan)
    sockets.append((f'socket.cargo.{row}',point(CX,(y0+y1)/2,1.50)))

bevel_box('cargo | recessed intermodule seam',(75,474,440,486),.12,2.94,
          bevel=.045,corner=.35,mat=graphite)

# Retaining rails are raised on top and below, with physical end returns.
# Six amber-gold locks on each visible face correspond to the reference.
for label,x in [('port',171),('starboard',343)]:
    bounds=(x-14,314,x+14,649)
    bevel_box('rail.'+label+' | dorsal retention rail',bounds,2.96,3.19,
              bevel=.045,corner=.07,mode=rail_mapping)
    bevel_box('rail.'+label+' | ventral retention rail',bounds,-.15,.06,
              bevel=.045,corner=.07,mode=bottom_mapping)
    for endpoint,py in [('forward',315),('aft',648)]:
        bevel_box('rail.'+label+' | '+endpoint+' return',
                  (x-13,py-3,x+13,py+3),.01,3.16,
                  bevel=.035,corner=.04,mat=graphite)
    for index,py in enumerate([327,483,640],1):
        bounds=(x-10,py-16,x+10,py+16)
        bevel_box(f'lock.{label}.{index} | top gold cargo clamp',bounds,3.12,3.32,
                  bevel=.042,corner=.07,mode=rail_mapping)
        bevel_box(f'lock.{label}.{index} | belly gold cargo clamp',bounds,-.27,-.11,
                  bevel=.032,corner=.07,mode=bottom_mapping)

# A tapered forward pressure cabin, with shaped roof/canopy planes and a
# separate mechanical neck. The window is sampled from the approved artwork.
loft('bridge | faceted forward pressure cabin',[
    (61,14,1.65,2.06,2.27),(84,22,1.61,2.21,2.42),
    (110,30,1.55,2.28,2.62),(144,47,1.48,2.47,2.95),
    (174,56,1.41,2.63,3.19),(194,59,1.32,2.70,3.34),
    (244,64,1.13,2.63,3.30),(281,50,1.17,2.22,2.76),
    (301,30,1.32,1.91,2.25),
],mode=cabin_mapping)

pod=[(304,185),(321,194),(342,235),(341,259),(320,280),(307,270),(298,232)]
for side,label in [(-1,'port'),(1,'starboard')]:
    slab('bridge.'+label+' | sensor and maneuvering fairing',pod,
         lambda x,y:interpolate(y,[(185,2.55),(228,2.74),(259,2.55),(280,2.11)]),
         1.17,side=side,mode=cabin_mapping,bevel=.06)
    beam('bridge.'+label+' | neck brace',point(CX+side*26,278,1.58),
         point(CX+side*48,326,1.08),.12,.20,graphite)
bevel_box('bridge | small brass nose latch',(250,65,264,86),2.16,2.40,
          bevel=.03,corner=.05,mat=brass)
bevel_box('bridge | neck collar',(229,295,285,315),1.27,2.07,
          bevel=.06,corner=.10,mode=cabin_mapping)
bevel_box('bridge | dorsal service grille frame',(244,239,272,287),2.51,2.94,
          bevel=.055,corner=.08,mode=rail_mapping)
bevel_box('bridge | ventral inspection cover',(239,187,276,258),1.10,1.21,
          bevel=.026,corner=.08,mode=bottom_mapping)
sockets.append(('socket.bridge',point(CX,171,3.36)))

# Central tail fairing and structural arms connecting the twin drives.
loft('stern | central drive-service fairing',[
    (644,50,.40,2.09,2.49),(663,41,.48,2.07,2.36),
    (697,33,.69,1.83,2.13),(726,20,.91,1.62,1.80),
],mode='auto')
for side,label,px in [(-1,'port',134),(1,'starboard',382)]:
    beam('engine.'+label+' | forward mount',point(CX+side*43,647,1.42),
         point(px,682,1.49),.25,.38,graphite)
    beam('engine.'+label+' | aft brace',point(CX+side*28,704,1.24),
         point(px,715,1.32),.16,.22,silver)
    prefix='engine.'+label
    z=1.49;radius=1.055
    def casing_mapping(n,center,px=px):
        if n[2]>.28:return atlas,top_uv
        if n[2]<-.28:return ventral,bottom_uv
        if abs(n[1])>.75:return graphite,top_uv
        def uv(v):
            py=CY-v[1]*S
            return ((1635+(py-672)*105/141)/2048,(341-(v[2]-.36)*64)/2048)
        return atlas,uv
    radial(prefix+' | stepped armored engine casing',px,
           [(671,.59),(686,.88),(704,radius),(758,radius),
            (775,.86),(786,.71)],z,mode=casing_mapping,segments=32)
    # Independent metal collars and open nozzle throat with an inset disc.
    ring(prefix+' | drive coupling',px,685,z,.90,.78,.10,graphite)
    ring(prefix+' | machined exhaust shoulder',px,780,z,.825,.66,.10,silver)
    radial(prefix+' | continuous tapered nozzle jacket',px,
           [(781,.82),(787,.72),(803,.66),(810,.66)],z,
           mat=dark,cap=False,segments=32)
    ring(prefix+' | dark nozzle lip',px,807,z,.66,.525,.11,dark)
    ring(prefix+' | inner nozzle wall',px,800,z,.528,.435,.24,silver)
    ring(prefix+' | brass aperture retaining ring',px,795,z,.44,.406,.065,brass)
    def exhaust_mapping(n,center,px=px,z=z):
        if abs(n[1])<.90:return dark,top_uv
        def uv(v):
            return ((px+(CX+v[0]*S-px)/S*18/.412)/2048,
                    (803-(v[2]-z)*6.0/.412)/2048)
        return atlas,uv
    radial(prefix+' | recessed textured ion aperture',px,
           [(794,.412),(794.6,.412)],z,mode=exhaust_mapping,segments=48)
    sockets.append((f'socket.engine.{label}',point(px,813,z)))

export()
