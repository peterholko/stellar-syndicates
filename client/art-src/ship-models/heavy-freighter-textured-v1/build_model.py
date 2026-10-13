"""Heavy Freighter: a solid armored transport with four bays and eight engines.

Explicit component meshes and reference-derived UVs; Python/NumPy only.
The four large roof plates are hatches in a continuous enclosed pressure hull.
"""
from geometry import *


def armor_mapping(n,c):
    if n[2]<-.30:return ventral,bottom_uv
    if n[2]>.30:return atlas,top_uv
    if abs(n[0])>=abs(n[1]):return atlas,side_uv
    # Reuse the unoccluded front bulkhead for aft cargo walls. Actual rear
    # engine apertures are projected only onto the physical engine discs.
    return atlas,front_uv

def shallow_plate(n,c):
    if n[2]<-.30:return ventral,bottom_uv
    if n[2]>.30:return atlas,top_uv
    return graphite,top_uv

def belly_plate(n,c):
    if abs(n[2])>.30:return ventral,bottom_uv
    return graphite,top_uv

def clamp_mapping(n,c):
    if n[2]<-.30:return ventral,bottom_uv
    if n[2]>.30:return atlas,top_uv
    return brass,top_uv

def bridge_mapping(n,c):
    py=CY-c[1]*S
    if n[2]<-.30:return ventral,bottom_uv
    if n[1]>.23 and py<136:
        def uv(v):return ((1363-v[0]*77)/2048,(279-v[2]*40)/2048)
        return atlas,uv
    if abs(n[0])>.58:
        def uv(v):
            py=CY-v[1]*S
            x=interpolate(py,[(83,967),(109,1000),(142,1055),(194,1102),(259,1150)])
            return (x/2048,(768-v[2]*34.3)/2048)
        return atlas,uv
    if n[2]>.25:
        def uv(v):
            py=CY-v[1]*S
            py=interpolate(py,[(83,83),(110,110),(136,146),(180,180),(259,259)])
            return ((CX+v[0]*S)/2048,py/2048)
        return atlas,uv
    return graphite,top_uv

def sensor_mapping(n,c):
    if n[2]<-.30:return ventral,bottom_uv
    if n[2]>.30:return atlas,top_uv
    if abs(n[0])>.45:
        def uv(v):return ((292+(v[2]-.77)/1.42*16)/2048,(CY-v[1]*S)/2048)
        return atlas,uv
    return graphite,top_uv

def spine_mapping(n,c):
    if n[2]<-.30:return ventral,bottom_uv
    if n[2]>.30:return atlas,top_uv
    return graphite,top_uv

def reactor_mapping(n,c):
    if n[2]<-.30:return ventral,bottom_uv
    if n[2]>.30:return atlas,top_uv
    if n[1]<-.4:
        def uv(v):return ((1363+v[0]*72)/2048,(568-v[2]*61)/2048)
        return atlas,uv
    if abs(n[0])>.5:
        def uv(v):
            py=CY-v[1]*S
            return ((1600+(py-586)*.54)/2048,(806-v[2]*58)/2048)
        return atlas,uv
    return graphite,top_uv

def crane_mapping(bounds):
    x0,y0,x1,y1=bounds
    def mapping(n,c):
        if n[2]<-.30:return ventral,bottom_uv
        if n[2]>.30:return atlas,top_uv
        if abs(n[0])>.55:
            def uv(v):
                py=CY-v[1]*S
                return ((1215+(py-334)/195*309)/2048,(765-(v[2]-.68)/.9*56)/2048)
            return atlas,uv
        return graphite,top_uv
    return mapping


# One closed shield-shaped pressure hull. It is deliberately solid under the
# central service spine and between all four recessed cargo-hatch stations.
outline=[(133,222),(381,222),(476,330),(476,539),(385,642),
         (345,652),(169,652),(126,642),(38,539),(38,330)]
slab('hull | continuous armored pressure body',outline,2.14,.12,
     bevel=.17,mode=armor_mapping)

# Four raised hatch lids and separately modeled ventral access plates. The
# pressure hull closes the space between the lids instead of exposing a gap.
for label,x0,x1 in [('port',83,231),('starboard',283,431)]:
    for row,(y0,y1) in enumerate([(281,424),(453,595)],1):
        prefix=f'cargo.{label}.{row}'
        bevel_box(prefix+' | armored dorsal hatch',(x0,y0,x1,y1),2.105,2.295,
                  bevel=.065,corner=.45,mode=shallow_plate)
        bevel_box(prefix+' | ventral service plate',(x0+3,y0+3,x1-3,y1-3),
                  -.025,.17,bevel=.055,corner=.42,mode=belly_plate)
        cx=(x0+x1)/2
        bevel_box(prefix+' | roof restraint',(cx-5.5,y0+6,cx+5.5,y1-6),
                  2.275,2.345,bevel=.016,corner=.035,mode=shallow_plate)
        for i,py in enumerate([y0+9,y1-9],1):
            bevel_box(prefix+f' | gold lock {i}',(cx-7,py-5,cx+7,py+5),
                      2.318,2.45,bevel=.02,corner=.04,mode=clamp_mapping)
        sockets.append((f'socket.cargo.{label}.{row}',point(cx,(y0+y1)/2,2.30)))

# Raised shoulder plates follow the armored perimeter. The broad side walls
# keep the true SIDE artwork, including vents and panel seams.
shoulder=[(285,233),(380,233),(419,280),(386,284),(353,263),(288,264)]
flank=[(433,302),(466,337),(466,533),(425,580),(424,545),(442,517),(442,358)]
aft_plate=[(318,608),(383,608),(420,570),(433,589),(383,636),(321,636)]
for side,label in [(-1,'port'),(1,'starboard')]:
    slab('armor.'+label+' | forward shoulder',shoulder,2.225,2.10,
         side=side,bevel=.035,mode=shallow_plate)
    slab('armor.'+label+' | perimeter cheek',flank,2.23,2.08,
         side=side,bevel=.035,mode=shallow_plate)
    slab('armor.'+label+' | aft shoulder',aft_plate,2.225,2.10,
         side=side,bevel=.035,mode=shallow_plate)
    for py in (337,526):
        x=448 if side>0 else 66
        bevel_box('armor.'+label+f' | navigation lamp {py}',(x-1.8,py-7,x+1.8,py+7),
                  2.215,2.265,bevel=.01,corner=.03,mat=dark)
        beam('light.'+label+f' | cyan navigation strip {py}',point(x,py-4,2.27),
             point(x,py+4,2.27),.023,.018,cyan)

# Small real trusses sit in the transverse recess between the two hatch rows.
# They have a continuous hull surface behind them, rather than a through-hole.
for label,x0,x1 in [('port',92,226),('starboard',288,422)]:
    for name,z in [('dorsal',2.22),('ventral',.045)]:
        a,b,c,d=point(x0,431,z),point(x1,431,z),point(x1,447,z),point(x0,447,z)
        for i,(start,end) in enumerate([(a,b),(b,c),(c,d),(d,a),(a,c),(b,d)],1):
            beam(f'structure.{label}.{name} | transverse brace {i}',start,end,.055,.065,graphite)

# Narrow continuous centerline service keel and its raised machinery housings.
bevel_box('spine | continuous service keel',(242,222,272,704),.07,2.33,
          bevel=.055,corner=.095,mode=spine_mapping)
for name,bounds,lo,hi in [
    ('forward distribution cap',(239,258,275,362),2.30,2.50),
    ('midship radiator housing',(238,393,276,499),2.31,2.53),
    ('aft cargo coupler',(241,552,273,610),2.29,2.50),
]:
    bevel_box('spine | '+name,bounds,lo,hi,bevel=.04,corner=.12,mode=spine_mapping)
for row,(a,b) in enumerate([(364,391),(501,550)],1):
    for i,(x0,py0,x1,py1) in enumerate([(245,a,269,b),(269,a,245,b)],1):
        beam(f'structure.spine | diagonal {row}.{i}',point(x0,py0,2.34),point(x1,py1,2.34),.06,.075,silver)
bevel_box('spine | ventral inspection rail',(245,260,269,635),-.06,.09,
          bevel=.025,corner=.075,mode=belly_plate)

# Slender projecting command prow. Frontal glazing is mapped only once across
# its leading slope; the adjoining roof starts behind the painted windshield.
loft('bridge | faceted command prow',[
    (83,11,.97,1.54,1.68),(99,18,.83,1.84,2.02),
    (113,24,.76,2.05,2.38),(136,27,.69,2.19,2.60),
    (160,29,.61,2.13,2.58),(202,33,.54,2.06,2.54),
    (236,30,.61,1.98,2.38),(260,20,.89,1.91,2.15),
],mode=bridge_mapping)
sensor=[(289,137),(298,146),(309,167),(307,190),(295,207),(289,199)]
for side,label in [(-1,'port'),(1,'starboard')]:
    slab('bridge.'+label+' | sensor fairing',sensor,
         lambda x,y:interpolate(y,[(137,1.83),(167,2.05),(190,1.89),(207,1.65)]),
         .77,side=side,bevel=.05,mode=sensor_mapping)
bevel_box('bridge | ventral access cover',(247,144,267,207),.43,.65,
          bevel=.035,corner=.10,mode=belly_plate)
radial('sensor | forward axial probe',CX,[(54,.045),(63,.055),(77,.067),(87,.123)],
       1.25,mat=graphite,segments=24)
ring('sensor | probe collar',CX,77,1.25,.133,.09,.085,brass)
sockets.append(('socket.bridge',point(CX,135,2.64)))

# Folded cargo cranes on the outside of the armor, not weapon mounts. Real
# rails, hinges and hydraulic pistons give their silhouettes depth from below.
for side,label,cx in [(-1,'port',26),(1,'starboard',488)]:
    bounds=(cx-8,334,cx+8,530)
    bevel_box('crane.'+label+' | folded longitudinal carrier',bounds,.72,1.49,
              bevel=.055,corner=.13,mode=crane_mapping(bounds))
    for rail,x in [('outer',cx+side*7),('inner',cx-side*5)]:
        beam('crane.'+label+' | '+rail+' folded arm',point(x,357,1.04),
             point(x,508,1.04),.11,.14,silver if rail=='outer' else graphite)
    for i,py in enumerate([349,384,468,512],1):
        # Lateral mounting foot intersects the hull wall and carrier.
        hx=side*4.35
        beam(f'crane.{label} | mounting foot {i}',(hx,(CY-py)/S,.80),
             point(cx,py,.97),.14,.20,graphite)
        bound=(cx-9,py-7,cx+9,py+7)
        bevel_box(f'crane.{label} | hinge block {i}',bound,.59,1.67,
                  bevel=.035,corner=.07,mode=crane_mapping(bound))
        # Exposed gold ring is a hinge face and the slim silver rod its shaft.
        for x,z,kind in [(cx+side*3,1.51,'upper'),(cx+side*3,.73,'lower')]:
            radial(f'crane.{label} | {kind} hinge {i}',x,[(py-2,.105),(py+2,.105)],
                   z,mat=brass,segments=24)
    for i,(a,b) in enumerate([(337,375),(480,526)],1):
        px=cx+side*3
        radial(f'crane.{label} | hydraulic barrel {i}',px,[(a,.10),(b,.10)],
               1.63,mat=brass,segments=24)
        radial(f'crane.{label} | hydraulic rod {i}',px,[(a-7,.045),(b+8,.045)],
               1.63,mat=silver,segments=24)
    sockets.append((f'socket.crane.{label}',point(cx,432,1.1)))

# A narrower stern drive pack is physically joined to the pressure hull.
stern_outline=[(171,629),(343,629),(375,680),(375,724),
               (351,759),(163,759),(139,724),(139,680)]
slab('drive | armored eight-engine support',stern_outline,1.90,.11,
     bevel=.115,mode=armor_mapping)
loft('stern | central reactor-service fairing',[
    (571,16,.27,2.21,2.50),(601,25,.11,2.22,2.59),
    (634,36,.07,2.39,2.74),(673,30,.03,2.13,2.62),
    (705,25,.04,1.97,2.44),(750,17,.17,1.87,2.16),
    (776,13,.38,1.60,1.85),
],mode=reactor_mapping)
sockets.append(('socket.reactor',point(CX,648,2.77)))

# Eight aft-facing nozzles: four lateral columns, two vertical tiers. The
# aperture art is sampled from the BACK view at each actual exhaust center.
for column,px,ref_x in [('port.outer',177,1215),('port.inner',221,1285),
                        ('starboard.inner',293,1440),('starboard.outer',337,1510)]:
    for tier,z,ref_y in [('upper',1.50,494),('lower',.52,551)]:
        prefix=f'engine.{column}.{tier}'
        def casing_mapping(n,c):
            if n[2]<-.28:return ventral,bottom_uv
            if n[2]>.28:return atlas,top_uv
            if abs(n[1])>.75:return graphite,top_uv
            def uv(v):
                py=CY-v[1]*S
                return ((1635+(py-704)*1.03)/2048,(807-v[2]*62)/2048)
            return atlas,uv
        radial(prefix+' | stepped armored casing',px,
               [(704,.29),(719,.38),(735,.44),(758,.44),(766,.40)],
               z,mode=casing_mapping,segments=32)
        ring(prefix+' | brass thermal collar',px,761,z,.439,.36,.065,brass)
        ring(prefix+' | machined exhaust shoulder',px,768,z,.422,.332,.08,silver)
        radial(prefix+' | tapered nozzle jacket',px,
               [(766,.415),(775,.397),(790,.375),(796,.375)],z,
               mat=dark,cap=False,segments=32)
        ring(prefix+' | outer exhaust lip',px,795,z,.377,.306,.085,dark)
        ring(prefix+' | recessed throat',px,787,z,.309,.252,.16,silver)
        def exhaust_mapping(n,c,px=px,ref_x=ref_x,ref_y=ref_y,z=z):
            if abs(n[1])<.9:return dark,top_uv
            def uv(v):
                return ((ref_x+(CX+v[0]*S-px)/S*22/.245)/2048,
                        (ref_y-(v[2]-z)*22/.245)/2048)
            return atlas,uv
        radial(prefix+' | recessed textured ion aperture',px,
               [(784,.245),(784.5,.245)],z,mode=exhaust_mapping,segments=32)
        sockets.append((f'socket.engine.{column}.{tier}',point(px,798,z)))

export()
