"""Titan v4: layered capital hull, two engine blocks, four oval drives.

The approved top drawing sets the planform. Heights are reconciled with SIDE;
all downward armor surfaces use the independent BOTTOM drawing. No Blender.
"""
from geometry import *


def deck(py):
    return interpolate(py,[(46,1.94),(175,2.23),(320,2.43),(430,2.53),
                           (550,2.52),(640,2.37),(700,2.10),(747,1.65)])


def turret_mapping(n,c):
    if n[2]>.35:return atlas,top_uv
    return graphite,top_uv


def armor_mapping(n,c):
    if n[2]<-.30:return ventral,bottom_uv
    if n[2]>.30:return atlas,top_uv
    if abs(n[0])>.55:return atlas,side_uv
    if n[1]>.55:return atlas,front_uv
    if n[1]<-.55:return atlas,back_uv
    return graphite,top_uv


def stern_mapping(n,c):
    if n[2]<-.30:return ventral,bottom_uv
    if n[2]>.30:return atlas,top_uv
    if abs(n[0])>.55:return atlas,side_uv
    if n[1]<-.55:return atlas,back_uv
    return graphite,top_uv


# A closed, faceted pressure hull. Its stern contracts between the drive blocks,
# leaving the rear cutouts genuinely open rather than filling them with a card.
loft('hull | continuous armored pressure body',[
    (46,43,1.18,1.79,1.94),(61,53,1.07,1.95,2.08),
    (108,61,.95,2.00,2.14),(174,69,.80,2.05,2.23),
    (207,79,.73,2.08,2.29),(286,102,.62,2.13,2.38),
    (322,126,.53,2.18,2.44),(350,148,.44,2.18,2.48),
    (428,160,.33,2.13,2.53),(447,147,.30,2.08,2.51),
    (521,160,.23,2.05,2.52),(552,167,.24,2.08,2.51),
    (584,186,.31,2.07,2.46),(629,196,.42,2.04,2.40),
    (648,160,.43,1.99,2.33),(662,148,.43,1.96,2.25),
    (700,46,.42,1.76,2.10),(735,39,.54,1.59,1.87),
    (747,29,.69,1.34,1.65),
],mode=hull_mapping)

# Major dorsal plating follows the artwork's central strip, aft terraces and
# separated shoulder shelves. Their bevels remain real at every detail level.
slab('deck | central foredeck armor',[(222,48),(298,48),(316,85),
     (332,205),(342,282),(318,319),(208,319),(185,283),(203,175),(209,83)],
     lambda x,y:deck(y)+.075,lambda x,y:deck(y)-.08,bevel=.04,mode=armor_mapping)
slab('deck | aft central armor terrace',[(213,321),(308,321),(335,350),
     (334,475),(350,523),(343,614),(302,646),(216,644),(183,612),(176,516),
     (193,476),(188,355)],lambda x,y:deck(y)+.10,
     lambda x,y:deck(y)-.10,bevel=.055,mode=armor_mapping)
loft('keel | deep segmented ventral armor',[
    (113,27,.88,1.17,1.34),(218,39,.54,.94,1.11),
    (324,66,.11,.67,.91),(430,76,-.11,.54,.77),
    (530,66,-.12,.49,.78),(609,48,.03,.61,.88),
    (690,28,.23,.69,.97),(742,18,.55,.97,1.20)
],mode=armor_mapping)

for side,label in [(-1,'port'),(1,'starboard')]:
    def px(x):return CX+side*(x-CX)
    slab(f'armor.{label} | forward shelf',[(345,208),(374,210),(388,301),
         (377,319),(359,286)],lambda x,y:2.12+(y-208)*.0009,1.57,
         side=side,bevel=.065,mode=armor_mapping)
    slab(f'armor.{label} | broad mid shoulder',[(384,320),(409,329),(428,348),
         (434,431),(414,441),(383,418)],2.20,1.01,
         side=side,bevel=.07,mode=armor_mapping)
    slab(f'armor.{label} | aft stepped shoulder',[(401,451),(418,457),
         (427,522),(443,546),(459,571),(465,641),(438,651),
         (430,597),(401,569)],2.18,.86,side=side,bevel=.08,mode=armor_mapping)
    slab(f'armor.{label} | lower shoulder flange',[(390,459),(416,465),
         (424,523),(438,552),(452,580),(456,628),(424,638),(407,561)],
         .99,.45,side=side,bevel=.065,mode=armor_mapping)

    # Continuous longitudinal spars and shorter segmented upper edge rails.
    beam(f'rail.{label} | forward embedded structural spar',point(px(330),82,1.89),
         point(px(360),305,2.10),.14,.22,graphite)
    beam(f'rail.{label} | aft longitudinal spar',point(px(389),330,2.23),
         point(px(405),639,2.22),.11,.17,graphite)
    for i,(x,y0,y1,z) in enumerate([(334,96,153,2.02),(346,181,254,2.12),
             (396,353,417,2.34),(403,475,540,2.33),(419,576,632,2.28)],1):
        beam(f'fitting.{label} | rail cap {i}',point(px(x),y0,z),point(px(x),y1,z),
             .065,.06,silver)
        for py in (y0+3,y1-3):
            bevel_box(f'fitting.{label} | rail lock {i}.{py}',
                (px(x)-2,py-2,px(x)+2,py+2),z-.075,z+.065,
                bevel=.018,corner=.025,mat=brass)

    # Solid textured plates set back behind an armor frame make real recesses.
    # Their flanks use the corresponding native SIDE hangar artwork.
    for i,(x,a,b,lo,hi) in enumerate([(391,381,418,1.12,1.69),
                                    (417,499,551,.91,1.59)],1):
        wallx=px(x)
        beam(f'sidewall.{label}.{i} | recessed machinery wall',point(wallx,a,lo),
             point(wallx,b,lo),.04,.07,graphite)
        for z,part in [(lo,'sill'),(hi,'lintel')]:
            beam(f'sidewall.{label}.{i} | {part}',point(wallx,a,z),point(wallx,b,z),.11,.12,graphite)
        for py,part in [(a,'front jamb'),(b,'rear jamb')]:
            beam(f'sidewall.{label}.{i} | {part}',point(wallx,py,lo),point(wallx,py,hi),.11,.11,silver)
        for j in range(7):
            py=a+3+j*(b-a-6)/6
            beam(f'fitting.{label}.{i} | radiator fin {j}',point(wallx+side*.016,py,lo+.08),
                 point(wallx+side*.016,py,hi-.08),.024,.05,dark)

    # Lower service panels use their own BOTTOM-view UV coordinates.
    for i,(x,a,b,z) in enumerate([(337,275,315,.47),(358,366,416,.20),
              (342,474,525,.17),(355,560,615,.21)],1):
        x=px(x)
        bevel_box(f'keel.{label}.{i} | ventral maintenance armor',
            (x-13,a,x+13,b),z,z+.21,bevel=.035,corner=.08,mode=armor_mapping)

    # Navigation lamps are thin embedded surfaces, not oversized neon trim.
    for i,(x,py,z) in enumerate([(328,158,1.85),(431,414,1.81),(455,618,1.71)],1):
        beam(f'light.{label}.{i} | dark lamp seat',point(px(x),py-3,z),
             point(px(x),py+3,z),.06,.07,dark)
        beam(f'light.{label}.{i} | cyan slit',point(px(x)+side*.035,py-2,z),
             point(px(x)+side*.035,py+2,z),.019,.033,cyan)

# Low centerline command housing and an aft service fairing; neither is a tall
# new bridge tower. Their separate undersides receive bottom artwork too.
loft('bridge | recessed command blister',[
    (287,9,2.23,2.43,2.50),(305,14,2.27,2.57,2.69),
    (327,20,2.31,2.65,2.86),(345,18,2.31,2.63,2.82),
    (359,11,2.30,2.47,2.61)],mode=armor_mapping)
for side in [-1,1]:
    beam('bridge | cyan window '+str(side),point(CX+side*10,317,2.69),
         point(CX+side*14,338,2.72),.022,.035,cyan)
loft('stern | central service fairing',[
    (635,31,1.46,2.28,2.53),(651,39,1.22,2.32,2.69),
    (676,41,.91,2.28,2.72),(708,33,.76,2.12,2.52),
    (731,25,.75,1.83,2.13),(747,18,.80,1.46,1.72)
],mode=stern_mapping)
bevel_box('stern | dorsal service access',(246,648,274,683),2.59,2.78,
          bevel=.04,corner=.08,mode=armor_mapping)
sockets.extend([('socket.bridge',point(CX,333,2.91)),
                ('socket.reactor',point(CX,672,2.82))])


def engine_block(name,cx):
    """Closed body with a recessed aft cavity; the back is not a solid cap."""
    def section(py,hw,lo,hi,bevel):
        coords=[(-hw+bevel,lo),(hw-bevel,lo),(hw,lo+bevel/S),
                (hw,hi-bevel/S),(hw-bevel,hi),(-hw+bevel,hi),
                (-hw,hi-bevel/S),(-hw,lo+bevel/S)]
        return [point(cx+x,py,z) for x,z in coords]
    rows=[(641,47,.46,2.49,9),(653,57,.19,2.78,11),
          (674,59,.06,2.85,12),(749,58,.06,2.70,11),
          (780,53,.10,2.65,10),(791,45,.18,2.53,9),
          (791,38,.40,2.30,7),(773,38,.40,2.30,7)]
    v=[p for row in rows for p in section(*row)]
    f=[tuple(reversed(range(8)))]
    for k in range(len(rows)-1):
        f.extend((k*8+i,k*8+(i+1)%8,(k+1)*8+(i+1)%8,(k+1)*8+i) for i in range(8))
    f.append(tuple(range(len(v)-8,len(v))))
    def mapping(n,c):
        py=CY-c[1]*S
        if n[2]<-.30:return ventral,bottom_uv
        if py>770 and n[1]<-.45:
            # The reference end view compresses each engine housing separately.
            # Register the two drawn exhausts to the two physical nozzle centers.
            reference_center = 1059 if cx<CX else 1568
            def rear_uv(v):
                return ((reference_center+(v[0]-(cx-CX)/S)*(60/1.05))/2048,
                        (472-(v[2]-1.37)*(37/.84))/2048)
            return atlas,rear_uv
        return armor_mapping(n,c)
    mesh(name,v,f,mode=mapping)


def oval_shell(name,px,rows,z,mat=dark,mode='auto',cap=False,segments=48):
    """Rounded-rectangle cross section along the fore/aft axis."""
    def shape(v):return math.copysign(abs(v)**(2/3.0),v)
    v=[]
    for py,rx,rz in rows:
        for i in range(segments):
            angle=2*math.pi*i/segments
            v.append(point(px+shape(math.cos(angle))*rx*S,py,z+shape(math.sin(angle))*rz))
    f=[]
    for k in range(len(rows)-1):
        f.extend((k*segments+i,k*segments+(i+1)%segments,
                  (k+1)*segments+(i+1)%segments,(k+1)*segments+i) for i in range(segments))
    if cap:f.extend([tuple(reversed(range(segments))),tuple(range(len(v)-segments,len(v)))])
    return mesh(name,v,f,mode=mode,mat=mat,smooth=True)


for side,label,cx,ref_centers in [(-1,'port',140,(1029,1089)),
                                  (1,'starboard',380,(1538,1598))]:
    engine_block(f'engineblock.{label} | hollow armored housing',cx)
    bevel_box(f'engineblock.{label} | dorsal armor lid',(cx-48,655,cx+48,742),
              2.64,2.86,bevel=.065,corner=.20,mode=armor_mapping)
    bevel_box(f'engineblock.{label} | ventral service plate',(cx-40,665,cx+40,754),
              -.015,.18,bevel=.035,corner=.14,mode=armor_mapping)
    for edge in [-1,1]:
        beam(f'fitting.engineblock.{label}.{edge} | side stiffener',
             point(cx+edge*55,677,1.09),point(cx+edge*54,762,1.09),.085,.14,graphite)
        for py in [690,731,760]:
            beam(f'fitting.engineblock.{label}.{edge}.{py} | brass latch',
                 point(cx+edge*55,py,.80),point(cx+edge*55,py,1.07),.045,.06,brass)
    for i,(px,refx) in enumerate(zip([cx-21,cx+21],ref_centers),1):
        prefix=f'engine.{label}.{i}';z=1.37
        oval_shell(prefix+' | layered nozzle jacket',px,[
            (775,.38,.99),(781,.41,1.02),(789,.40,1.01),(792,.36,.98),
            (792,.29,.88),(781,.27,.84),(775,.30,.89),(775,.38,.99)
        ],z,mat=dark,segments=48)
        oval_shell(prefix+' | machined oval lip',px,[
            (789,.389,.99),(791,.372,.971),(791,.345,.936),
            (789,.362,.955),(789,.389,.99)],z,mat=silver,segments=48)
        oval_shell(prefix+' | brass inner collar',px,[
            (784,.300,.893),(785,.297,.888),(785,.278,.856),
            (784,.281,.861),(784,.300,.893)],z,mat=brass,segments=40)
        def emission_mapping(n,c,px=px,refx=refx,z=z):
            if abs(n[1])<.8:return dark,top_uv
            def uv(v):
                return ((refx+(CX+v[0]*S-px)/S*13/.27)/2048,
                        (472-(v[2]-z)*37/.84)/2048)
            return atlas,uv
        oval_shell(prefix+' | recessed textured plasma aperture',px,
                   [(778,.27,.84),(779,.27,.84)],z,mat=atlas,
                   mode=emission_mapping,cap=True,segments=48)
        # Subtle emitter bars are recessed behind the outer lip.
        for rib in range(9):
            height=z-.66+rib*.165
            beam(prefix+f' | emitter vane {rib}',point(px-7.5,780,height),
                 point(px+7.5,780,height),.012,.020,cyan)
        sockets.append((f'socket.engine.{label}.{i}',point(px,794,z)))


# Four weapon assemblies are exported separately from the static material
# groups. Optimized exports add pivots at these same mount centers.
for label,side in [('port',-1),('starboard',1)]:
    for i,(offset,py) in enumerate([(80,387),(91,561)],1):
        px=CX+side*offset;z=deck(py)+.12;prefix=f'turret.{label}.{i}'
        # Vertical axis cylinder (rounded base) built explicitly in model Z.
        count=40;v=[]
        for radius,h in [(.25,z-.01),(.34,z+.07),(.33,z+.15),(.24,z+.22)]:
            v.extend(point(px+math.cos(k*2*math.pi/count)*radius*S,
                           py+math.sin(k*2*math.pi/count)*radius*S,h) for k in range(count))
        f=[tuple(reversed(range(count)))]
        for k in range(3):f.extend((k*count+j,k*count+(j+1)%count,
                   (k+1)*count+(j+1)%count,(k+1)*count+j) for j in range(count))
        f.append(tuple(range(3*count,4*count)))
        mesh(prefix+' | rotating pedestal',v,f,mat=graphite,smooth=True)
        bevel_box(prefix+' | armored turret housing',(px-10,py-14,px+10,py+12),
                  z+.11,z+.43,bevel=.075,corner=.13,mode=turret_mapping)
        for barrel,dx in enumerate([-4,4],1):
            radial(prefix+f' | cannon barrel {barrel}',px+dx,
                   [(py-34,.05),(py-25,.056),(py-12,.075),(py-3,.085)],
                   z+.32,mat=graphite,segments=40)
            ring(prefix+f' | cannon collar {barrel}',px+dx,py-13,z+.32,
                 .091,.061,.060,brass)
            ring(prefix+f' | muzzle rim {barrel}',px+dx,py-34,z+.32,
                 .061,.031,.055,silver)
            sockets.append((f'socket.weapon.{label}.{i}.{barrel}',point(px+dx,py-35,z+.32)))
        sockets.append((f'socket.turret.{label}.{i}',point(px,py,z+.06)))

# Keep small radar fittings compact so the reference's low profile stays clear.
for i,(px,py,z) in enumerate([(247,311,2.70),(273,311,2.70),(CX,340,2.87)],1):
    beam(f'fitting.sensor.{i} | short mast',point(px,py,z),point(px,py,z+.11),.035,.035,graphite)

export()
