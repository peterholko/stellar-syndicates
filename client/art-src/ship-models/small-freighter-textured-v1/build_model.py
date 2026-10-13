"""Build the five-view Small Freighter as solid, textured geometry; no Blender.

The top controls planform, the bottom gets a separate ventral projection, and
component-local side/end projections reconcile the illustration's view scales.
"""
from geometry import *


def bottom_mapping(n, center):
    if abs(n[2]) > .28: return ventral, bottom_uv
    return graphite, top_uv


def rail_mapping(n, center):
    if n[2] > .32: return atlas, top_uv
    if n[2] < -.32: return ventral, bottom_uv
    return graphite, top_uv


def lock_mapping(n, center):
    if n[2] > .32: return atlas, top_uv
    if n[2] < -.32: return ventral, bottom_uv
    return brass, top_uv


def spine_mapping(n, center):
    if n[2] < -.30: return ventral, bottom_uv
    if n[2] > .30: return atlas, top_uv
    return graphite, top_uv


def cargo_mapping(bounds, row):
    x0, y0, x1, y1 = bounds
    side_panel = (1080, 1325) if row == 1 else (1343, 1587)
    def mapping(n, center):
        if n[2] < -.34: return ventral, bottom_uv
        if n[2] > .34: return atlas, top_uv
        if abs(n[0]) > abs(n[1]):
            def uv(v):
                py = CY-v[1]*S
                return ((side_panel[0]+(py-y0)/(y1-y0)*(side_panel[1]-side_panel[0]))/2048,
                        (795-v[2]*57.5)/2048)
            return atlas, uv
        # The source's rear engines obscure part of the cargo end plates.
        # Use the unobstructed matching front end for these hidden surfaces.
        def uv(v):
            px = CX+v[0]*S
            return ((1664-(px-x0)/(x1-x0)*207)/2048,
                    (298-v[2]*59.0)/2048)
        return atlas, uv
    return mapping


def cabin_mapping(n, center):
    if n[2] < -.30: return ventral, bottom_uv
    py=CY-center[1]*S
    if n[1] > .50 and py < 158:
        def uv(v):
            return ((1348-v[0]*70)/2048,(307-v[2]*41.5)/2048)
        return atlas, uv
    if abs(n[0]) > .55:
        def uv(v):
            py = CY-v[1]*S
            x = interpolate(py, [(84,953),(130,983),(175,1024),(220,1062),(268,1090)])
            return (x/2048,(780-v[2]*41.5)/2048)
        return atlas, uv
    if n[2] > .28: return atlas, top_uv
    def uv(v):
        return ((1348-v[0]*70)/2048,(307-v[2]*41.5)/2048)
    return atlas, uv


def stern_mapping(n, center):
    if n[2] < -.30: return ventral, bottom_uv
    if n[2] > .30: return atlas, top_uv
    if n[1] < -.45:
        def uv(v): return ((1348+v[0]*58)/2048,(558-v[2]*56)/2048)
        return atlas, uv
    return graphite, top_uv


# Slim, continuous load-bearing spine. The open channels beside it are actual
# empty space, crossed only by the four cargo attachment frames below.
loft('hull | continuous structural spine', [
    (246,19,.60,1.61,1.88),(274,25,.32,1.64,1.91),
    (383,26,.27,1.58,1.90),(414,29,.23,1.67,2.04),
    (460,30,.23,1.71,2.09),(493,24,.27,1.52,1.87),
    (597,25,.32,1.47,1.80),(635,34,.31,1.73,2.03),
    (666,38,.35,1.68,1.98),
], mode=spine_mapping)
for name,bounds,low,high in [
    ('forward service cover',(247,281,271,366),1.87,2.02),
    ('central power housing',(245,413,273,477),1.99,2.20),
    ('rear radiator cover',(246,507,272,590),1.77,1.94),
]:
    bevel_box('spine | '+name,bounds,low,high,bevel=.04,corner=.065,mode=rail_mapping)
bevel_box('spine | ventral utility hatch',(246,354,272,469),.17,.29,
          bevel=.03,corner=.075,mode=bottom_mapping)

# Four independently solid cargo containers: two consecutive sections per side.
for side,label,x0,x1 in [(-1,'port',54,207),(1,'starboard',311,464)]:
    rail_x=(x0+x1)/2
    for row,(y0,y1) in enumerate([(220,392),(400,639)],1):
        prefix=f'cargo.{label}.{row}'
        bounds=(x0,y0,x1,y1)
        mapping=cargo_mapping(bounds,row)
        bevel_box(prefix+' | beveled pressure container',bounds,.02,2.38,
                  bevel=.13,corner=.44,mode=mapping)
        bevel_box(prefix+' | dorsal loading panel',
                  (x0+19,y0+18,x1-19,y1-16),2.34,2.43,
                  bevel=.032,corner=.09,mode=rail_mapping)
        bevel_box(prefix+' | ventral access hatch',
                  (x0+24,y0+23,x1-24,y1-20),-.055,.07,
                  bevel=.04,corner=.12,mode=bottom_mapping)
        light_stations=[304] if row==1 else [477,568]
        for i,py in enumerate(light_stations,1):
            x=x0 if side<0 else x1
            bevel_box(prefix+f' | running-light surround {i}',
                      (x-1,py-5,x+1,py+5),.82,1.20,
                      bevel=.017,corner=.018,mat=dark)
            beam(prefix+f' | cyan running light {i}',
                 point(x+side*1.2,py,.87),point(x+side*1.2,py,1.15),
                 .035,.055,cyan)
        sockets.append((f'socket.cargo.{label}.{row}',point(rail_x,(y0+y1)/2,1.20)))

    # Raised retention bands, wrapped returns and four matching lock stations.
    bevel_box('rail.'+label+' | dorsal retention rail',
              (rail_x-8,222,rail_x+8,637),2.40,2.56,
              bevel=.035,corner=.06,mode=rail_mapping)
    bevel_box('rail.'+label+' | ventral retention rail',
              (rail_x-8,222,rail_x+8,637),-.15,-.035,
              bevel=.028,corner=.06,mode=bottom_mapping)
    for name,py in [('forward',222),('aft',637)]:
        bevel_box('rail.'+label+' | '+name+' return',
                  (rail_x-8,py-2,rail_x+8,py+2),-.04,2.54,
                  bevel=.022,corner=.025,mat=graphite)
    for i,py in enumerate([226,386,406,632],1):
        bounds=(rail_x-8,py-9,rail_x+8,py+9)
        bevel_box(f'lock.{label}.{i} | top cargo clamp',bounds,2.53,2.71,
                  bevel=.038,corner=.055,mode=lock_mapping)
        bevel_box(f'lock.{label}.{i} | bottom cargo clamp',bounds,-.24,-.11,
                  bevel=.029,corner=.055,mode=lock_mapping)

    # X braces are physical metal beams with clear air around and between them.
    a,b=sorted([CX+side*23, x1 if side<0 else x0])
    for i,(front,rear) in enumerate([(288,340),(538,588)],1):
        corners=[point(a,front,1.28),point(b,front,1.28),
                 point(b,rear,1.28),point(a,rear,1.28)]
        for k in range(4):
            beam(f'structure.{label}.{i} | frame edge {k+1}',corners[k],corners[(k+1)%4],.105,.25,silver)
        beam(f'structure.{label}.{i} | diagonal A',corners[0],corners[2],.067,.12,silver)
        beam(f'structure.{label}.{i} | diagonal B',corners[1],corners[3],.067,.12,silver)
        edge=x1 if side<0 else x0
        bevel_box(f'structure.{label}.{i} | gold cargo cleat',
                  (edge-3.2,front-2,edge+3.2,rear+2),.99,1.57,
                  bevel=.04,corner=.035,mat=brass)

# The forward pressure cabin has a low pointed nose, slanted glazing, a raised
# roof and a mechanical neck. The source roof/window panels are projected onto
# those physical planes, rather than placed on an isolated flat card.
loft('bridge | faceted forward pressure cabin',[
    (87,12,.96,1.47,1.76),(105,22,.77,1.71,2.12),
    (131,32,.59,2.09,2.81),(151,39,.50,2.39,3.06),
    (177,46,.38,2.20,3.00),(219,48,.32,1.96,2.74),
    (245,38,.42,1.76,2.24),(268,20,.62,1.65,1.91),
],mode=cabin_mapping)
pod=[(292,139),(302,154),(316,191),(316,222),(304,246),(295,232),(291,189)]
for side,label in [(-1,'port'),(1,'starboard')]:
    slab('bridge.'+label+' | sensor and maneuvering fairing',pod,
         lambda x,y:interpolate(y,[(139,2.04),(177,2.40),(219,2.09),(246,1.69)]),
         .51,side=side,mode=cabin_mapping,bevel=.052)
    beam('bridge.'+label+' | neck brace',point(CX+side*20,254,1.01),
         point(CX+side*23,286,1.0),.10,.18,graphite)
bevel_box('bridge | neck collar',(237,260,281,278),.61,1.85,
          bevel=.06,corner=.10,mode=spine_mapping)
bevel_box('bridge | ventral maintenance hatch',(247,171,271,209),.265,.37,
          bevel=.028,corner=.07,mode=bottom_mapping)
radial('sensor | axial forward probe',CX,[(57,.074),(61,.104),(87,.104),(94,.17)],
       1.42,mat=graphite,segments=24)
ring('sensor | gold probe collar',CX,80,1.42,.111,.075,.075,brass)
sockets.append(('socket.bridge',point(CX,151,3.08)))

# Central drive fairing and the shallow U-shaped engine yoke.
loft('stern | central drive-service fairing',[
    (602,29,.37,1.72,2.08),(625,40,.33,1.84,2.21),
    (655,48,.29,1.76,2.23),(692,42,.36,1.77,2.18),
    (724,30,.51,1.56,1.89),(739,19,.75,1.39,1.60),
],mode=stern_mapping)
for side,label,px in [(-1,'port',148),(1,'starboard',370)]:
    beam('structure.'+label+' | drive yoke',point(CX+side*34,637,1.18),
         point(px,667,1.18),.28,.36,graphite)
    beam('structure.'+label+' | drive support',point(CX+side*36,676,1.15),
         point(px,706,1.15),.18,.24,silver)
    prefix='engine.'+label
    z=1.16
    def casing_mapping(n,center):
        if n[2]>.28:return atlas,top_uv
        if n[2]<-.28:return ventral,bottom_uv
        if abs(n[1])>.75:return graphite,top_uv
        def uv(v):
            py=CY-v[1]*S
            return ((1594+(py-645)*.87)/2048,(797-v[2]*56)/2048)
        return atlas,uv
    radial(prefix+' | stepped armored engine casing',px,
           [(646,.43),(658,.74),(683,1.055),(739,1.055),(759,.88),(773,.70)],
           z,mode=casing_mapping,segments=40)
    ring(prefix+' | forward drive coupling',px,655,z,.70,.57,.12,graphite)
    ring(prefix+' | brass heat shield band',px,760,z,.867,.76,.075,brass)
    ring(prefix+' | machined exhaust shoulder',px,775,z,.79,.65,.10,silver)
    radial(prefix+' | continuous tapered nozzle jacket',px,
           [(775,.785),(787,.70),(803,.66),(809,.66)],z,
           mat=dark,cap=False,segments=40)
    ring(prefix+' | outer nozzle lip',px,807,z,.66,.54,.12,dark)
    ring(prefix+' | inner nozzle throat',px,799,z,.543,.438,.24,silver)
    ring(prefix+' | aperture retaining ring',px,796,z,.442,.410,.06,brass)
    reference_x=1233 if side<0 else 1465
    def exhaust_mapping(n,center,px=px,reference_x=reference_x):
        if abs(n[1])<.9:return dark,top_uv
        def uv(v):
            return ((reference_x+(CX+v[0]*S-px)/S*23/.413)/2048,
                    (524-(v[2]-z)*23/.413)/2048)
        return atlas,uv
    radial(prefix+' | recessed textured ion aperture',px,
           [(794.4,.413),(795,.413)],z,mode=exhaust_mapping,segments=48)
    sockets.append((f'socket.engine.{label}',point(px,812,z)))

export()
