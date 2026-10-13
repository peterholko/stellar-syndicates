"""Corvette reconstructed from the approved five-view sheet, without Blender.

Pixel-space contours make the relationship to the reference inspectable.
Authoring uses Z-up; export converts to Y-up, -Z forward.
"""
from geometry import *

# The pressure hull and shoulders are closed volumes, not front/back cards.
loft('hull | continuous armored pressure body', [
    (76,14,1.16,1.35,1.53),(105,33,.88,1.61,1.80),
    (145,46,.49,1.87,2.04),(212,62,.28,1.93,2.15),
    (278,67,.17,1.82,2.12),(341,61,.11,1.75,2.05),
    (413,61,.07,1.72,1.98),(454,76,.12,1.65,1.90),
    (507,73,.23,1.63,1.88),(554,54,.28,1.51,1.77),
    (626,47,.22,1.48,1.75),(693,37,.30,1.46,1.65),
    (736,30,.42,1.22,1.44),(755,21,.55,1.02,1.16)])

shoulder=[(319,142),(356,185),(481,251),(510,276),(523,303),
          (522,347),(440,398),(387,426),(349,406),(328,321)]
leading=[(355,196),(482,258),(506,279),(503,300),(459,294),
         (350,258),(334,221)]
aft=[(351,317),(447,292),(464,357),(506,348),(422,401),
     (388,419),(350,394)]
def shoulder_roof(x,y):
    return interpolate(y,[(142,1.84),(235,2.10),(303,2.05),(347,1.99),(426,1.52)])
def shoulder_belly(x,y):
    return interpolate(y,[(142,.68),(240,.44),(347,.34),(426,.50)])

for label,side in [('port',-1),('starboard',1)]:
    slab(f'armor.{label} | closed forward shoulder',shoulder,shoulder_roof,shoulder_belly,side,bevel=.13)
    slab(f'armor.{label} | leading shoulder plate',leading,
         lambda x,y:shoulder_roof(x,y)+.085,lambda x,y:shoulder_roof(x,y)-.03,side,bevel=.025)
    slab(f'armor.{label} | aft shoulder plate',aft,
         lambda x,y:shoulder_roof(x,y)+.07,lambda x,y:shoulder_roof(x,y)-.04,side,bevel=.027)
    # Deep ventral armor follows the separate underside drawing.
    belly=[(334,225),(419,231),(500,289),(503,344),(423,398),(367,410),(338,342)]
    slab(f'keel.{label} | shoulder service plating',belly,
         lambda x,y:shoulder_belly(x,y)+.06,
         lambda x,y:shoulder_belly(x,y)-.11,side,bevel=.045)
    # Small raised planar bars sit over actual side-intake artwork.
    for i,py in enumerate([304,315,326,337],1):
        x=CX+side*241
        beam(f'intake.{label}.{i} | titanium grille',point(x,py,.78),point(x,py,1.66),.028,.033,silver)
    beam(f'intake.{label} | cyan marker',point(CX+side*234,324,1.18),point(CX+side*234,324,1.50),.045,.034,cyan)

# Layered center armor, with the vent well built as a real recess.
slab('deck | forward armored spine',[(262,78),(290,78),(309,139),(328,232),
     (311,297),(300,326),(254,326),(240,296),(223,232),(244,139)],
     lambda x,y:interpolate(y,[(78,1.57),(160,2.24),(232,2.38),(326,2.17)]),
     lambda x,y:interpolate(y,[(78,1.49),(160,2.04),(232,2.16),(326,2.0)]),bevel=.038)
slab('deck | port vent surround',[(244,334),(261,334),(261,371),(253,420),
     (254,503),(239,516),(231,450),(231,390)],2.03,1.86,bevel=.04)
slab('deck | starboard vent surround',[(306,334),(289,334),(289,371),(297,420),
     (296,503),(311,516),(319,450),(319,390)],2.03,1.86,bevel=.04)
bevel_box('vent | dorsal recessed intake',(261,374,291,415),1.70,1.89,bevel=.035,mat=dark)
for i in range(7):
    beam(f'vent.dorsal.{i} | radiator slat',point(264,379+i*5,1.925),point(288,379+i*5,1.925),.030,.026,graphite)
loft('spine | tapered aft armor',[(422,25,1.74,1.92,2.02),(505,38,1.62,1.89,2.00),
     (556,32,1.51,1.81,1.94),(626,27,1.50,1.79,1.90),(698,26,1.43,1.69,1.80)])
loft('keel | continuous ventral service spine',[(132,22,.39,.50,.61),
     (210,29,.09,.23,.34),(282,32,-.03,.10,.28),(360,33,-.10,.09,.20),
     (430,28,-.05,.15,.25),(510,28,.08,.27,.36),(596,27,.05,.24,.33),
     (690,25,.15,.34,.43),(749,20,.43,.53,.62)])
for i,(py,w,length) in enumerate([(282,26,80),(398,24,35),(485,23,29),(550,22,70),(653,21,65)],1):
    z=interpolate(py,[(132,.39),(360,-.10),(430,-.05),(510,.08),(596,.05),(690,.15)])
    bevel_box(f'keel.panel.{i} | ventral maintenance armor',(CX-w,py-length/2,CX+w,py+length/2),z-.075,z+.055,bevel=.025)
for label,side in [('port',-1),('starboard',1)]:
    # The narrow service spine is flanked by shaped armor at the engine roots.
    slab(f'armor.{label} | aft service cheek',[(324,413),(348,425),(354,492),
         (332,543),(315,531)],1.69,.42,side,bevel=.07)
    slab(f'armor.{label} | swept engine root',[(330,545),(364,572),(385,625),
         (349,669),(317,646)],lambda x,y:interpolate(y,[(545,1.5),(590,1.7),(669,1.3)]),
         .43,side,bevel=.075)
    # Mechanical braces remain visible across the negative space beside the tail.
    for i,(a,b) in enumerate([((347,532,.87),(381,597,1.0)),((327,650,.77),(365,690,.84))],1):
        a=point(CX+side*(a[0]-CX),a[1],a[2]);b=point(CX+side*(b[0]-CX),b[1],b[2])
        beam(f'brace.{label}.{i} | dark coupling',a,b,.27,.27,graphite)
        aa=np.array(a);bb=np.array(b)
        beam(f'brace.{label}.{i} | brass actuator',aa+(bb-aa)*.20,aa+(bb-aa)*.73,.29,.29,brass)
    slab(f'armor.{label} | actuator support frame',[(350,413),(393,427),(384,482),(350,507)],
         1.43,.47,side,bevel=.06)
    for py in (453,469,484):
        px=CX+side*91
        radial(f'service.{label}.{py} | side actuator',px,[(py-7,.067),(py+7,.067)],.89,mat=brass,segments=24)
loft('stern | central service fairing',[(697,26,.38,1.33,1.59),(727,28,.40,1.24,1.43),(755,19,.55,1.02,1.16)])

# Low armored command blister, with a small forward-facing canopy.
loft('bridge | low command blister',[(202,13,2.21,2.53,2.73),
     (215,23,2.20,2.62,2.91),(235,22,2.18,2.59,2.83),(251,16,2.15,2.37,2.48)])
bevel_box('bridge | canopy forward glass',(263,206,289,217),2.60,2.69,bevel=.020,mat=dark)
bevel_box('bridge | canopy cyan slit',(266,207,286,212),2.685,2.707,bevel=.004,mat=cyan)
for label,side in [('port',-1),('starboard',1)]:
    px=CX+side*28
    beam(f'bridge.{label} | brass flank rail',point(px,164,2.24),point(px,202,2.39),.12,.14,brass)
sockets.append(('socket.bridge',point(CX,223,2.94)))
sockets.append(('socket.reactor',point(CX,605,1.0)))

# Two symmetric cylindrical engines. Their mouths are genuine recessed tubes,
# with separately mapped exhaust faces, not end caps painted onto a box.
for label,side in [('port',-1),('starboard',1)]:
    px=CX+side*122
    ez=1.04
    radial(f'enginecase.{label} | closed pressure housing',px,
           [(581,.35),(604,.47),(624,.78),(659,1.05),(709,1.07),(739,.86)],
           ez,segments=32,mode=lambda n,c:(graphite,top_uv) if abs(n[1])>.70 else hull_mapping(n,c))
    # Armor panels sit above and below the rounded pressure casing.
    panel=[(px-21,613),(px+21,613),(px+43,652),(px+50,706),(px+38,726),
           (px+25,699),(px-25,699),(px-38,726),(px-50,706),(px-43,652)]
    slab(f'enginecase.{label} | upper armor cowl',panel,
         lambda x,y:ez+interpolate(y,[(613,.59),(653,.96),(706,1.02),(726,.82)])-.12*abs(x-px)/50,
         lambda x,y:ez+interpolate(y,[(613,.46),(653,.83),(706,.89),(726,.69)])-.12*abs(x-px)/50,
         bevel=.023)
    slab(f'enginecase.{label} | lower service cowl',panel,
         lambda x,y:ez-interpolate(y,[(613,.46),(653,.83),(706,.89),(726,.69)])+.12*abs(x-px)/50,
         lambda x,y:ez-interpolate(y,[(613,.59),(653,.96),(706,1.02),(726,.82)])+.12*abs(x-px)/50,
         bevel=.023)
    radial(f'engine.{label} | forward collar',px,[(581,.38),(588,.43),(603,.43),(610,.49)],ez,mat=graphite,segments=48)
    radial(f'engine.{label} | hollow stepped nozzle',px,
           [(728,.89),(741,.85),(750,.75),(765,.70),(784,.63),
            (788,.61),(788,.52),(774,.52),(751,.58),(733,.67),(728,.89)],
           ez,mat=graphite,segments=64,cap=False)
    ring(f'engine.{label} | outer machined rim',px,785,ez,.64,.54,.062,silver)
    ring(f'engine.{label} | brass inner ring',px,778,ez,.55,.48,.044,brass)
    for i,py in enumerate([738,751,765],1):
        r={738:.87,751:.76,765:.71}[py]
        ring(f'engine.{label}.{i} | nozzle band',px,py,ez,r+.025,r-.035,.043,dark)
    # Full rear artwork is used locally at its own center, preventing front/
    # back projections from printing unrelated engine mouths onto hull plating.
    uvcenter=1265 if side<0 else 1461
    def exhaust_uv(v,cx=uvcenter,worldx=(px-CX)/S):
        return ((cx+(v[0]-worldx)*49)/2048,(512-(v[2]-ez)*49)/2048)
    radial(f'engine.{label} | recessed textured plasma aperture',px,[(772,.516),(773,.516)],ez,
           mode=lambda n,c,uv=exhaust_uv:(atlas,uv) if abs(n[1])>.7 else (graphite,top_uv),segments=64)
    for i in range(8):
        angle=i*math.pi/4
        dx,dz=math.cos(angle)*.77,math.sin(angle)*.77
        beam(f'engine.{label}.rib.{i} | nozzle rib',point(px+dx*S,744,ez+dz),
             point(px+dx*.80*S,779,ez+dz*.80),.023,.033,silver)
    sockets.append((f'socket.engine.{label}',point(px,789,ez)))

# Four separately named assemblies survive merging and simplification. The LOD
# exporter adds each mount's yaw pivot and parents its weapon sockets correctly.
for label,side in [('port',-1),('starboard',1)]:
    for i,(py,offset) in enumerate([(247,149),(365,146)],1):
        px=CX+side*offset
        deck=shoulder_roof(px,py)+.08
        prefix=f'turret.{label}.{i}'
        count=48;vertices=[]
        rows=[(.33,deck-.015),(.44,deck+.055),(.43,deck+.13),(.34,deck+.20)]
        for radius,z in rows:
            vertices.extend(point(px+math.cos(j*2*math.pi/count)*radius*S,
                                  py+math.sin(j*2*math.pi/count)*radius*S,z) for j in range(count))
        faces=[tuple(reversed(range(count)))]
        for k in range(len(rows)-1):
            faces.extend((k*count+j,k*count+(j+1)%count,(k+1)*count+(j+1)%count,(k+1)*count+j) for j in range(count))
        faces.append(tuple(range(len(vertices)-count,len(vertices))))
        mesh(prefix+' | rotating pedestal',vertices,faces,mat=graphite,smooth=True)
        # Map only the armored housing top. Its sides are real dark plating.
        bevel_box(prefix+' | armored turret housing',(px-15,py-14,px+15,py+11),deck+.16,deck+.45,
                  bevel=.065,corner=.09,mode=lambda n,c:(atlas,top_uv) if n[2]>.35 else (graphite,top_uv))
        # A brass bearing ring is horizontal; use the same closed radial profile
        # with coordinates rotated from axial Y to vertical Z.
        count=40;v=[]
        for radius,z in [(.445,deck+.065),(.445,deck+.11),(.40,deck+.11),(.40,deck+.065)]:
            v.extend(point(px+math.cos(j*2*math.pi/count)*radius*S,py+math.sin(j*2*math.pi/count)*radius*S,z) for j in range(count))
        f=[]
        for k in range(4):
            f.extend((k*count+j,k*count+(j+1)%count,((k+1)%4)*count+(j+1)%count,((k+1)%4)*count+j) for j in range(count))
        mesh(prefix+' | brass bearing ring',v,f,mat=brass,smooth=True)
        for barrel,dx in enumerate([-7,7],1):
            bx=px+dx;bz=deck+.40
            radial(prefix+f' | barrel {barrel}',bx,[(py-59,.054),(py-53,.06),(py-26,.061),(py-11,.085),(py-3,.085)],bz,mat=graphite,segments=40)
            ring(prefix+f' | collar {barrel}',bx,py-13,bz,.092,.066,.071,brass)
            ring(prefix+f' | muzzle {barrel}',bx,py-59,bz,.064,.031,.067,silver)
            sockets.append((f'socket.weapon.{label}.{i}.{barrel}',point(bx,py-61,bz)))
        sockets.append((f'socket.turret.{label}.{i}',point(px,py,deck+.07)))

export()
