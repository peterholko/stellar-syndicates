"""Reconstruct the Large Freighter's twin keels and five-view surface artwork.

Y-up/-Z-forward glTF and editable OBJ; direct Python/NumPy mesh construction.
The central channel is open, with load paths only through the two end yokes.
"""
from geometry import *


def bottom_mapping(n,c):
    if abs(n[2])>.28:return ventral,bottom_uv
    return graphite,top_uv

def rail_mapping(n,c):
    if n[2]>.32:return atlas,top_uv
    if n[2]<-.32:return ventral,bottom_uv
    return graphite,top_uv

def clamp_mapping(n,c):
    if n[2]>.32:return atlas,top_uv
    if n[2]<-.32:return ventral,bottom_uv
    return brass,top_uv

def cargo_mapping(bounds,row):
    x0,y0,x1,y1=bounds
    u0,u1=[(1146,1321),(1328,1478),(1486,1638)][row-1]
    def mapping(n,c):
        if n[2]<-.32:return ventral,bottom_uv
        if n[2]>.32:return atlas,top_uv
        if abs(n[0])>abs(n[1]):
            def uv(v):
                py=CY-v[1]*S
                return ((u0+(py-y0)/(y1-y0)*(u1-u0))/2048,(799-v[2]*52.5)/2048)
            return atlas,uv
        # Engines obscure part of the rear end in the sheet. The matching front
        # end plate supplies those occluded areas, without painting an engine
        # onto a cargo wall.
        def uv(v):
            px=CX+v[0]*S
            return ((1702-(px-x0)/(x1-x0)*207)/2048,(327-v[2]*52.5)/2048)
        return atlas,uv
    return mapping

def keel_mapping(n,c):
    if n[2]<-.30:return ventral,bottom_uv
    if n[2]>.30:return atlas,top_uv
    return graphite,top_uv

def bow_mapping(center_x):
    def mapping(n,c):
        if n[2]<-.30:return ventral,bottom_uv
        if n[2]>.30:return atlas,top_uv
        if abs(n[0])>abs(n[1]):
            def uv(v):
                py=CY-v[1]*S
                return ((1260+(py-148)*.55)/2048,(789-v[2]*48)/2048)
            return atlas,uv
        def uv(v):
            px=CX+v[0]*S
            return ((1598-(px-center_x)*1.12)/2048,(324-v[2]*51)/2048)
        return atlas,uv
    return mapping

def cabin_mapping(n,c):
    if n[2]<-.30:return ventral,bottom_uv
    py=CY-c[1]*S
    if n[1]>.20 and py<130:
        def uv(v):return ((1378-v[0]*70)/2048,(295-v[2]*34.5)/2048)
        return atlas,uv
    if abs(n[0])>.55:
        def uv(v):
            py=CY-v[1]*S
            x=interpolate(py,[(74,1018),(104,1045),(152,1087),(215,1136),(240,1152)])
            return (x/2048,(767-v[2]*35)/2048)
        return atlas,uv
    if n[2]>.28:
        # The windshield belongs to the leading slope. Start the adjoining
        # roof behind it so the plan artwork does not repeat the glazing.
        def uv(v):
            py=CY-v[1]*S
            py=interpolate(py,[(74,74),(104,104),(124,137),(153,157),(195,195),(238,238)])
            return ((CX+v[0]*S)/2048,py/2048)
        return atlas,uv
    def uv(v):return ((1378-v[0]*70)/2048,(295-v[2]*34.5)/2048)
    return atlas,uv

def sensor_mapping(n,c):
    if n[2]<-.30:return ventral,bottom_uv
    if n[2]>.28:return atlas,top_uv
    if abs(n[0])>.4:
        def uv(v):
            return ((297+(v[2]-.47)/1.6*19)/2048,(CY-v[1]*S)/2048)
        return atlas,uv
    return graphite,top_uv

def coupler_mapping(n,c):
    if n[2]<-.30:return ventral,bottom_uv
    if n[2]>.30:return atlas,top_uv
    if abs(n[1])>.55:
        def uv(v):return ((267+v[0]*48)/2048,(298-(v[2]-.45)*28)/2048)
        return atlas,uv
    return graphite,top_uv

def stern_mapping(n,c):
    if n[2]<-.30:return ventral,bottom_uv
    if n[2]>.30:return atlas,top_uv
    if n[1]<-.45:
        def uv(v):return ((1378+v[0]*62)/2048,(568-v[2]*45)/2048)
        return atlas,uv
    return graphite,top_uv

def rear_yoke_mapping(side):
    def mapping(n,c):
        if n[2]<-.30:return ventral,bottom_uv
        if n[2]>.30:return atlas,top_uv
        if abs(n[1])>.55:
            # Reuse the yoke's panel artwork on its vertical bulkheads. The
            # BACK view has engines in front of this area, so projecting it
            # directly would paint false engine discs onto the crossbeam.
            def uv(v):
                px=CX+v[0]*S
                t=(px-170)/72 if side<0 else (364-px)/72
                return ((184+t*42)/2048,(751-(v[2]-.33)/1.26*43)/2048)
            return atlas,uv
        return graphite,top_uv
    return mapping


# Two independent load-bearing keels. These stay inside the container envelope
# and connect its three sections without filling the central channel.
for side,label,cx,x0,x1 in [(-1,'port',124,34,214),(1,'starboard',410,320,500)]:
    bevel_box('keel.'+label+' | load-bearing core',(cx-47,218,cx+47,742),
              .22,1.0,bevel=.08,corner=.14,mode=keel_mapping)
    loft('keel.'+label+' | tapered forward bow',[
        (148,10,.79,1.16,1.36),(165,22,.54,1.42,1.66),
        (193,46,.29,1.55,1.93),(228,75,.15,1.85,2.11),
        (240,87,.15,1.96,2.15),
    ],center=cx,mode=bow_mapping(cx))

    for row,(y0,y1) in enumerate([(237,389),(400,546),(557,700)],1):
        prefix=f'cargo.{label}.{row}'
        bounds=(x0,y0,x1,y1)
        bevel_box(prefix+' | beveled pressure container',bounds,.12,2.22,
                  bevel=.105,corner=.36,mode=cargo_mapping(bounds,row))
        bevel_box(prefix+' | dorsal loading panel',
                  (x0+17,y0+16,x1-17,y1-15),2.185,2.28,
                  bevel=.026,corner=.08,mode=rail_mapping)
        bevel_box(prefix+' | ventral access hatch',
                  (x0+22,y0+20,x1-22,y1-20),.015,.155,
                  bevel=.039,corner=.12,mode=bottom_mapping)
        x=x0 if side<0 else x1
        py=(y0+y1)/2
        bevel_box(prefix+' | running-light surround',(x-.8,py-4,x+.8,py+4),
                  .84,1.18,bevel=.014,corner=.015,mat=dark)
        beam(prefix+' | cyan running light',point(x+side*1.0,py,.89),
             point(x+side*1.0,py,1.13),.027,.046,cyan)
        sockets.append((f'socket.cargo.{label}.{row}',point(cx,py,1.1)))

    bevel_box('rail.'+label+' | dorsal retention rail',(cx-7,237,cx+7,698),
              2.235,2.40,bevel=.033,corner=.05,mode=rail_mapping)
    bevel_box('rail.'+label+' | ventral retention rail',(cx-7,237,cx+7,698),
              -.075,.09,bevel=.033,corner=.05,mode=bottom_mapping)
    for name,py in [('front',240),('rear',698)]:
        bevel_box('rail.'+label+' | '+name+' return',(cx-7,py-2,cx+7,py+2),
                  .025,2.39,bevel=.02,corner=.028,mat=graphite)
    for i,py in enumerate([250,378,415,536,570,685],1):
        bounds=(cx-6,py-10,cx+6,py+10)
        bevel_box(f'lock.{label}.{i} | dorsal gold lock',bounds,2.36,2.53,
                  bevel=.025,corner=.04,mode=clamp_mapping)
        bevel_box(f'lock.{label}.{i} | ventral gold lock',bounds,-.18,-.05,
                  bevel=.025,corner=.04,mode=clamp_mapping)

# A box truss on either side of the forward coupler. Top, underside and end
# diagonals are all real beams, so the bracing reads in both plan and elevation.
for side,label,a,b in [(-1,'port',207,243),(1,'starboard',291,327)]:
    front,rear,low,high=237,293,.48,1.94
    rings=[]
    for z in [low,high]:
        ring_points=[point(a,front,z),point(b,front,z),point(b,rear,z),point(a,rear,z)]
        rings.append(ring_points)
    edges=[]
    for r in rings:
        edges.extend((r[i],r[(i+1)%4]) for i in range(4))
        edges.extend([(r[0],r[2]),(r[1],r[3])])
    edges.extend((rings[0][i],rings[1][i]) for i in range(4))
    for i,j in [(0,1),(3,2)]:
        edges.extend([(rings[0][i],rings[1][j]),(rings[1][i],rings[0][j])])
    for i,(start,end) in enumerate(edges):
        beam(f'structure.forward.{label} | truss beam {i+1}',start,end,.085,.12,silver)
    edge=a if side<0 else b
    bevel_box('structure.forward.'+label+' | cargo anchor',
              (edge-2.4,front,edge+2.4,rear),.56,1.86,
              bevel=.03,corner=.025,mat=brass)
bevel_box('yoke | forward central coupler',(242,233,292,306),.43,2.31,
          bevel=.08,corner=.20,mode=coupler_mapping)

# Compact forward command cabin; it does not extend aft through the open gap.
loft('bridge | faceted command cabin',[
    (74,12,.81,1.43,1.60),(87,20,.66,1.79,1.98),
    (104,27,.56,2.15,2.57),(124,32,.51,2.24,2.72),
    (153,35,.42,2.14,2.68),(195,37,.43,1.89,2.55),
    (217,28,.55,1.79,2.17),(238,17,.71,1.62,1.85),
],mode=cabin_mapping)
pod=[(297,129),(307,138),(316,157),(316,181),(302,202),(296,189)]
for side,label in [(-1,'port'),(1,'starboard')]:
    slab('bridge.'+label+' | sensor fairing',pod,
         lambda x,y:interpolate(y,[(129,1.89),(157,2.05),(181,1.92),(202,1.61)]),
         .47,side=side,mode=sensor_mapping,bevel=.045)
bevel_box('bridge | ventral inspection hatch',(255,143,279,193),.345,.49,
          bevel=.027,corner=.08,mode=bottom_mapping)
radial('sensor | axial forward probe',CX,[(46,.066),(50,.089),(73,.089),(81,.139)],
       1.38,mat=graphite,segments=24)
ring('sensor | gold probe collar',CX,65,1.38,.103,.066,.073,brass)
sockets.append(('socket.bridge',point(CX,124,2.74)))

# Rear crossbeam and raised reactor cap connect the two keels at the stern.
for side,label,bounds in [(-1,'port',(170,704,242,756)),(1,'starboard',(292,704,364,756))]:
    bevel_box('yoke.'+label+' | transverse drive beam',bounds,.33,1.59,
              bevel=.075,corner=.16,mode=rear_yoke_mapping(side))
    for py in [710,749]:
        beam('structure.rear.'+label+f' | edge beam {py}',
             point(bounds[0],py,.48),point(bounds[2],py,.48),.13,.20,graphite)
loft('stern | central reactor-service fairing',[
    (663,15,.65,1.54,1.82),(678,29,.37,1.88,2.16),
    (704,36,.28,1.99,2.30),(745,31,.34,1.83,2.12),
    (769,21,.59,1.47,1.75),
],mode=stern_mapping)
sockets.append(('socket.reactor',point(CX,714,1.32)))

for label,cx in [('port',124),('starboard',410)]:
    loft('drive.'+label+' | armored twin-engine mount',[
        (696,22,.46,1.65,1.91),(717,25,.32,1.69,1.98),
        (753,21,.39,1.65,1.92),(780,15,.61,1.41,1.65),
    ],center=cx,mode=keel_mapping)
    beam('structure.drive.'+label+' | engine carrier',point(cx-27,753,.91),
         point(cx+27,753,.91),.21,.30,graphite)

# Four individual nozzles in two adjacent pairs, never stacked vertically.
engine_rows=[('port.outer',98,1203),('port.inner',157,1276),
             ('starboard.inner',377,1486),('starboard.outer',436,1561)]
for name,px,ref_x in engine_rows:
    prefix='engine.'+name
    z=.90
    def casing_mapping(n,c):
        if n[2]>.28:return atlas,top_uv
        if n[2]<-.28:return ventral,bottom_uv
        if abs(n[1])>.75:return graphite,top_uv
        def uv(v):
            py=CY-v[1]*S
            return ((1642+(py-730)*1.03)/2048,(793-v[2]*64)/2048)
        return atlas,uv
    radial(prefix+' | stepped armored engine casing',px,
           [(728,.35),(742,.50),(758,.57),(786,.57),(798,.49)],
           z,mode=casing_mapping,segments=36)
    ring(prefix+' | forward drive coupling',px,740,z,.505,.42,.10,graphite)
    ring(prefix+' | brass thermal collar',px,790,z,.557,.46,.07,brass)
    ring(prefix+' | machined exhaust shoulder',px,798,z,.512,.407,.08,silver)
    radial(prefix+' | tapered nozzle jacket',px,
           [(798,.51),(808,.47),(826,.445),(832,.445)],z,
           mat=dark,cap=False,segments=36)
    ring(prefix+' | outer nozzle lip',px,831,z,.445,.358,.10,dark)
    ring(prefix+' | inner nozzle throat',px,823,z,.360,.288,.19,silver)
    ring(prefix+' | aperture retaining ring',px,819,z,.291,.270,.045,brass)
    def exhaust_mapping(n,c,px=px,ref_x=ref_x):
        if abs(n[1])<.9:return dark,top_uv
        def uv(v):
            return ((ref_x+(CX+v[0]*S-px)/S*19/.273)/2048,
                    (552-(v[2]-z)*19/.273)/2048)
        return atlas,uv
    radial(prefix+' | recessed textured ion aperture',px,
           [(818,.273),(818.6,.273)],z,mode=exhaust_mapping,segments=40)
    sockets.append((f'socket.engine.{name}',point(px,835,z)))

export()
