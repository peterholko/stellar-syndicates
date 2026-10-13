"""Reconstruct the six-pod corporate freighter with editable solid geometry.

Run with Python/NumPy. Exports embedded-texture GLB and OBJ; no Blender required.
The planform follows the original 1254px top-down master. Depth is inferred
from the accompanying isometric reference. Pod faces use explicit box UVs.
"""
from mesh_core import *

def clamp(v,a=0,b=1):return max(a,min(b,v))

def octagon(x0,y0,x1,y1,c):
    return [(x0+c,y0),(x1-c,y0),(x1,y0+c),(x1,y1-c),
            (x1-c,y1),(x0+c,y1),(x0,y1-c),(x0,y0+c)]

def bevel_box(name,bounds,low,high,bevel=.18,corner=.24,mat=atlas,mode='top_bottom'):
    """Real three-dimensional corner and edge chamfers, with explicit UVs."""
    x0,y0,x1,y1=bounds
    bp=min(bevel*S,(x1-x0)/5,(y1-y0)/5)
    bz=min(bevel,(high-low)/3)
    cp=min(corner*S,(x1-x0)/3,(y1-y0)/3)
    rings=[(bp,low),(0,low+bz),(0,high-bz),(bp,high)]
    vertices=[]
    for inset,z in rings:
        for px,py in octagon(x0+inset,y0+inset,x1-inset,y1-inset,max(.05,min(cp-inset*.4,(x1-x0-2*inset)*.45,(y1-y0-2*inset)*.45))):
            vertices.append(point(px,py,z))
    faces=[tuple(reversed(range(8)))]
    for k in range(3):
        faces.extend((k*8+i,k*8+(i+1)%8,(k+1)*8+(i+1)%8,(k+1)*8+i) for i in range(8))
    faces.append(tuple(range(24,32)))
    return mesh(name,vertices,faces,mode,mat)

def pod_mapping(bounds,low,high,side):
    x0,y0,x1,y1=bounds
    def mapping(normal):
        # Continue the same panel seams, retaining band and edge trim below.
        if normal[2]<-.45:return ventral,top_uv
        if normal[2]>.35:return atlas,top_uv
        if abs(normal[1])>=abs(normal[0]):
            def uv(v):
                px=CX+v[0]*S
                fraction=clamp((v[2]-low)/(high-low))
                py=y0+45+(1-fraction)*(y1-y0-52)
                return px/2048,py/2048
        else:
            outer=normal[0]*side>0
            def uv(v):
                along=clamp((CY-v[1]*S-y0)/(y1-y0))
                up=clamp((v[2]-low)/(high-low))
                if outer:
                    px=x0+25+along*(x1-x0-50)
                else:
                    px=(x0+4+along*40) if side>0 else (x1-4-along*40)
                py=y0+35+(1-up)*(y1-y0-56)
                return px/2048,py/2048
        return atlas,uv
    return mapping

def hull_mapping(normal):
    if normal[2]<-.42:return ventral,top_uv
    if normal[2]>.35:return atlas,top_uv
    if abs(normal[0])>.55:
        def uv(v):
            sign=1 if v[0]>=0 else -1
            px=CX+sign*(29+19*(1-clamp(v[2]/2)))
            return px/2048,(CY-v[1]*S)/2048
        return atlas,uv
    return graphite,top_uv

def center_loft(name,sections,mode=hull_mapping,mat=atlas):
    # Pixel Y, half-width, lower Z, shoulder Z, roof Z.
    vertices=[]
    for py,w,low,shoulder,roof in sections:
        for x,z in [(-w*.68,roof),(w*.68,roof),(w,shoulder),
                    (w,low+.14),(w*.68,low),(-w*.68,low),(-w,low+.14),(-w,shoulder)]:
            vertices.append(point(CX+x,py,z))
    faces=[tuple(reversed(range(8)))]
    for k in range(len(sections)-1):
        faces.extend((k*8+i,k*8+(i+1)%8,(k+1)*8+(i+1)%8,(k+1)*8+i) for i in range(8))
    faces.append(tuple(range(len(vertices)-8,len(vertices))))
    return mesh(name,vertices,faces,mode,mat)

def beam(name,start,end,width,depth,mat=graphite):
    a,b=np.array(start,float),np.array(end,float)
    direction=unit(b-a)
    lateral=unit(np.cross(direction,np.array([0,0,1.])))
    up=unit(np.cross(lateral,direction))
    vertices=[]
    for center in [a,b]:
        for side,vertical in [(-1,-1),(1,-1),(1,1),(-1,1)]:
            vertices.append(center+lateral*side*width/2+up*vertical*depth/2)
    return mesh(name,vertices,[(0,3,2,1),(4,5,6,7),(0,1,5,4),(1,2,6,5),(2,3,7,6),(3,0,4,7)],mat=mat)

def radial(name,px,stations,zcenter,mat=atlas,cap=True,segments=40):
    vertices=[]
    for py,radius in stations:
        for i in range(segments):
            theta=2*math.pi*i/segments
            vertices.append(point(px+math.cos(theta)*radius*S,py,zcenter+math.sin(theta)*radius))
    faces=[]
    for k in range(len(stations)-1):
        faces.extend((k*segments+i,k*segments+(i+1)%segments,(k+1)*segments+(i+1)%segments,(k+1)*segments+i) for i in range(segments))
    if cap:faces.extend([tuple(reversed(range(segments))),tuple(range(len(vertices)-segments,len(vertices)))])
    return mesh(name,vertices,faces,mode='top',mat=mat,smooth=True)

def ring(name,px,py,z,outer,inner,depth,mat=dark):
    rows=[(py-depth*S/2,outer),(py+depth*S/2,outer),(py+depth*S/2,inner),(py-depth*S/2,inner),(py-depth*S/2,outer)]
    return radial(name,px,rows,z,mat,cap=False,segments=48)

# A continuous keel, raised service trunk and forward bridge support.
center_loft('01 | central structural keel',[
    (156,32,.08,1.02,1.50),(205,62,.04,1.12,1.82),(270,48,-.05,1.14,1.70),
    (320,44,-.12,1.08,1.74),(530,45,-.14,1.11,1.72),(745,43,-.14,1.06,1.66),
    (876,44,-.10,1.16,1.72),(940,66,-.06,1.30,2.00),
    (1040,64,-.04,1.14,1.81),(1140,42,.04,.94,1.40),(1176,18,.17,.66,.91),
])
bevel_box('02 | dorsal cargo-management spine',(594,294,659,866),1.68,2.02,.13,.17,mode=hull_mapping)
for name,bounds,top in [
    ('03 | forward navigation fairing',(577,172,677,272),2.12),
    ('04 | midship service hatch',(597,557,655,661),2.17),
    ('05 | aft drive control module',(580,914,675,1022),2.25),
]:bevel_box(name,bounds,1.62,top,.13,.20,mode=hull_mapping)

# Six individual containers, three pairs. Top textures stay at their native pixels.
rows=[(294,485),(532,727),(774,966)]
for row,(y0,y1) in enumerate(rows,1):
    mid=(y0+y1)/2
    for side,label,x0,x1 in [(-1,'port',240,544),(1,'starboard',710,1014)]:
        bounds=(x0,y0,x1,y1)
        low,high=-.25,1.88
        mapping=pod_mapping(bounds,low,high,side)
        prefix=f'cargo.{row}.{label}'
        bevel_box(prefix+' | chamfered freight pod',bounds,low,high,.22,.39,mode=mapping)
        pc=(x0+x1)/2
        # A real steel retention band wraps around roof, flanks and underside.
        band=[]
        yz=[(y0+15,high+.026),(y1-15,high+.026),(y1+1,high-.20),(y1+1,low+.20),
            (y1-15,low-.025),(y0+15,low-.025),(y0-1,low+.20),(y0-1,high-.20)]
        for px in [pc-9,pc+9]:
            band.extend(point(px,py,z) for py,z in yz)
        faces=[tuple(reversed(range(8))),tuple(range(8,16))]
        faces.extend((i,(i+1)%8,(i+1)%8+8,i+8) for i in range(8))
        mesh(prefix+' | steel retention band',band,faces,mode=mapping)
        for end,py in [('forward',y0+1),('aft',y1-1)]:
            bevel_box(prefix+' | '+end+' brass latch',(pc-7,py-7,pc+7,py+7),1.62,2.015,.035,.045,mat=brass)
            bevel_box(prefix+' | '+end+' latch inset',(pc-3,py-5,pc+3,py+5),1.99,2.025,.007,.016,mat=dark)
        # Open square dock frames with crossed braces, visible through the gaps.
        inner=x1 if side<0 else x0
        hull=CX+side*43
        first,last=sorted([hull,inner])
        front,back=mid-39,mid+39
        for name,a,b in [
            ('front rail',(first,front),(last,front)),('aft rail',(first,back),(last,back)),
            ('hull rail',(hull,front),(hull,back)),('pod rail',(inner,front),(inner,back)),
        ]:beam(prefix+' | dock '+name,point(*a,.69),point(*b,.69),.12,.26,graphite)
        beam(prefix+' | diagonal brace A',point(first,front,.69),point(last,back,.69),.072,.13,silver)
        beam(prefix+' | diagonal brace B',point(first,back,.69),point(last,front,.69),.072,.13,silver)
        # A small golden docking collar on the pod end of each connecting frame.
        bevel_box(prefix+' | docking collar',(inner-5,mid-29,inner+5,mid+29),.49,.98,.045,.035,mat=brass)
        # Physical cyan running light and inset frame on the outward pod face.
        outer=x1 if side>0 else x0
        face_bounds=(outer-1.0,mid-10,outer+1.0,mid+10)
        bevel_box(prefix+' | outboard light frame',face_bounds,.55,1.29,.035,.055,mat=dark)
        outside=outer+side*1.4
        frame=[point(outside,mid-2.6,.71),point(outside,mid+2.6,.71),
               point(outside,mid+2.6,1.13),point(outside,mid-2.6,1.13)]
        mesh(prefix+' | cyan running light',frame,[(0,1,2,3),(3,2,1,0)],mat=cyan)
        sockets.append((f'socket.cargo.{row}.{label}',point((x0+x1)/2,mid,.80)))

# Bridge: raised faceted cab, glazed band and compact sensor mast.
def cab_mapping(normal):
    # Use plain service armor below the cabin, avoiding a second painted canopy.
    if normal[2]<-.4:return ventral,panel_uv((593,72,661,167),(296,335,361,438))
    if normal[2]>.32:return atlas,top_uv
    def uv(v):
        height=clamp((v[2]-1.10)/1.48)
        if abs(normal[0])>.5:
            along=clamp((CY-v[1]*S-72)/95)
            px=599+along*55
            py=84+(1-height)*60
        else:
            px=599+clamp((v[0]+.48)/.96)*55
            py=79+(1-height)*68
        return px/2048,py/2048
    return atlas,uv
center_loft('bridge | pressure cabin',[
    (72,19,1.20,1.67,2.35),(88,27,1.17,1.85,2.56),
    (117,34,1.11,1.87,2.48),(150,32,1.09,1.58,2.02),(167,22,1.16,1.38,1.68),
],mode=cab_mapping)

def window_mapping(normal):
    if normal[2]<-.4:return ventral,panel_uv((605,72,649,79),(296,335,340,342))
    def uv(v):
        px=603+clamp((v[0]+.31)/.62)*46
        py=123+(1-clamp((v[2]-1.67)/.34))*23
        return px/2048,py/2048
    return atlas,uv
bevel_box('bridge | forward glazed viewport',(605,72,649,79),1.69,2.02,.026,.05,mode=window_mapping)
for side,label in [(-1,'port'),(1,'starboard')]:
    px=CX+side*44
    bevel_box('bridge | '+label+' sensor nacelle',(px-6,104,px+6,153),1.35,1.72,.035,.055,mode='top_bottom')
    x0,x1=(542,584) if side<0 else (670,712)
    bevel_box('bridge | '+label+' shoulder fairing',(x0,178,x1,262),.52,1.59,.12,.15,mode='top_bottom')
    beam('bridge | '+label+' running light',point(CX+side*59,193,1.61),point(CX+side*59,242,1.61),.032,.025,cyan)
mast=[point(623,72,2.17),point(631,72,2.17),point(628,29,2.62),
      point(624,29,2.62),point(623,72,2.67),point(631,72,2.67),point(628,29,3.02),point(624,29,3.02)]
mesh('bridge | compact navigation mast',mast,[(0,3,2,1),(4,5,6,7),(0,1,5,4),(1,2,6,5),(2,3,7,6),(3,0,4,7)],mat=graphite)
bevel_box('bridge | mast brass foot',(617,62,638,83),2.18,2.38,.026,.045,mat=brass)

# Broad aft machinery deck and four compact independent engine nozzles.
bevel_box('stern | main machinery deck',(450,1006,805,1128),-.11,1.21,.23,.34,mode='top_bottom')
for index,(px,outer,py_end) in enumerate([(500,True,1173),(568,False,1188),(686,False,1188),(755,True,1173)],1):
    prefix=f'engine.{index}'
    radius=.39 if outer else .43
    bevel_box(prefix+' | raised armored cowl',(px-25,991 if outer else 1018,px+25,1129),.71,1.53,.12,.15,mode='top_bottom')
    radial(prefix+' | titanium drive casing',px,[(1096,radius*.78),(1116,radius),(py_end-22,radius),(py_end-6,radius*.90)],.53)
    ring(prefix+' | brass collar',px,py_end-21,.53,radius+.035,radius-.022,.075,brass)
    ring(prefix+' | nozzle lip',px,py_end,.53,radius*.89,radius*.71,.095,dark)
    ring(prefix+' | internal throat',px,py_end-6,.53,radius*.715,radius*.58,.17,silver)
    radial(prefix+' | recessed ion aperture',px,[(py_end-7,radius*.60),(py_end-6,radius*.60)],.53,mat=cyan,segments=48)
    sockets.append((f'socket.engine.{index}',point(px,py_end+4,.53)))

export()
