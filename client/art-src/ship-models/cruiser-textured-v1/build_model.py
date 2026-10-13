"""Build the five-view Cruiser as editable textured solid meshes without Blender."""
from mesh_core import *

def clamp(v,a=0,b=1):return max(a,min(b,v))

def octagon(x0,y0,x1,y1,c):
    return [(x0+c,y0),(x1-c,y0),(x1,y0+c),(x1,y1-c),
            (x1-c,y1),(x0+c,y1),(x0,y1-c),(x0,y0+c)]

def slab(name,outline,top,bottom,side=1,mode='auto',mat=atlas,bevel=0):
    outline=mirrored(outline,side)
    if sum(a[0]*b[1]-b[0]*a[1] for a,b in zip(outline,outline[1:]+outline[:1]))>0:
        outline.reverse()
    n=len(outline)
    z=lambda f,x,y:f(x,y) if callable(f) else f
    if bevel:
        contour=np.array(outline,dtype=float)
        inset=[]
        for i,b in enumerate(contour):
            incoming=unit(b-contour[i-1]);outgoing=unit(contour[(i+1)%n]-b)
            # The contour is clockwise in pixel coordinates, hence right normals.
            n1=np.array([incoming[1],-incoming[0]])
            n2=np.array([outgoing[1],-outgoing[0]])
            move=(n1+n2)*bevel*S/max(.08,1+np.dot(n1,n2))
            length=np.linalg.norm(move)
            if length>bevel*S*3:move*=bevel*S*3/length
            inset.append(b+move)
        v=[]
        for ring,kind in [(inset,'low'),(contour,'lower'),(contour,'upper'),(inset,'high')]:
            for x,y in ring:
                lo,hi=z(bottom,x,y),z(top,x,y)
                b=min(bevel,(hi-lo)/3)
                height={'low':lo,'lower':lo+b,'upper':hi-b,'high':hi}[kind]
                v.append(point(x,y,height))
        faces=[tuple(reversed(range(n)))]
        for k in range(3):faces.extend((k*n+i,k*n+(i+1)%n,(k+1)*n+(i+1)%n,(k+1)*n+i) for i in range(n))
        faces.append(tuple(range(3*n,4*n)))
        return mesh(name,v,faces,mode,mat)
    v=[point(x,y,z(bottom,x,y)) for x,y in outline]+[point(x,y,z(top,x,y)) for x,y in outline]
    faces=[tuple(reversed(range(n))),tuple(range(n,n*2))]
    faces.extend((i,(i+1)%n,(i+1)%n+n,i+n) for i in range(n))
    return mesh(name,v,faces,mode,mat)

def bevel_box(name,bounds,low,high,bevel=.09,corner=.16,mat=atlas,mode='auto'):
    x0,y0,x1,y1=bounds
    bp=min(bevel*S,(x1-x0)/5,(y1-y0)/5)
    bz=min(bevel,(high-low)/3)
    cp=min(corner*S,(x1-x0)/3,(y1-y0)/3)
    rings=[(bp,low),(0,low+bz),(0,high-bz),(bp,high)]
    v=[]
    for inset,z in rings:
        c=max(.025,min(cp-inset*.4,(x1-x0-2*inset)*.45,(y1-y0-2*inset)*.45))
        v.extend(point(x,y,z) for x,y in octagon(x0+inset,y0+inset,x1-inset,y1-inset,c))
    f=[tuple(reversed(range(8)))]
    for k in range(3):f.extend((k*8+i,k*8+(i+1)%8,(k+1)*8+(i+1)%8,(k+1)*8+i) for i in range(8))
    f.append(tuple(range(24,32)))
    return mesh(name,v,f,mode,mat)

def loft(name,sections,center=CX,mode='auto',mat=atlas):
    # Y-pixel, half-width, belly, upper side edge, roof ridge.
    v=[]
    for py,w,low,edge,roof in sections:
        lip=min(.14,max(.018,(edge-low)*.24))
        for x,z in [(-w*.72,roof),(w*.72,roof),(w,edge),(w*.96,low+lip),
                    (w*.78,low),(-w*.78,low),(-w*.96,low+lip),(-w,edge)]:
            v.append(point(center+x,py,z))
    f=[tuple(reversed(range(8)))]
    for k in range(len(sections)-1):
        f.extend((k*8+i,k*8+(i+1)%8,(k+1)*8+(i+1)%8,(k+1)*8+i) for i in range(8))
    f.append(tuple(range(len(v)-8,len(v))))
    return mesh(name,v,f,mode,mat)

def beam(name,start,end,width,depth,mat=graphite):
    a,b=np.array(start,float),np.array(end,float)
    direction=unit(b-a)
    helper=np.array([0.,0.,1.]) if abs(direction[2])<.95 else np.array([1.,0.,0.])
    lateral=unit(np.cross(direction,helper));up=unit(np.cross(lateral,direction))
    v=[c+lateral*s*width/2+up*t*depth/2 for c in [a,b] for s,t in [(-1,-1),(1,-1),(1,1),(-1,1)]]
    return mesh(name,v,[(0,3,2,1),(4,5,6,7),(0,1,5,4),(1,2,6,5),(2,3,7,6),(3,0,4,7)],mat=mat)

def radial(name,px,stations,center_z,mat=atlas,mode='auto',cap=True,segments=48):
    v=[]
    for py,r in stations:
        v.extend(point(px+math.cos(i*2*math.pi/segments)*r*S,py,
                       center_z+math.sin(i*2*math.pi/segments)*r) for i in range(segments))
    f=[]
    for k in range(len(stations)-1):
        f.extend((k*segments+i,k*segments+(i+1)%segments,(k+1)*segments+(i+1)%segments,(k+1)*segments+i) for i in range(segments))
    if cap:f.extend([tuple(reversed(range(segments))),tuple(range(len(v)-segments,len(v)))])
    return mesh(name,v,f,mode,mat,smooth=True)

def ring(name,px,py,z,outer,inner,depth,mat=dark):
    rows=[(py-depth*S/2,outer),(py+depth*S/2,outer),(py+depth*S/2,inner),
          (py-depth*S/2,inner),(py-depth*S/2,outer)]
    return radial(name,px,rows,z,mat=mat,cap=False)

# Continuous lower hull; the shoulder armor sits above the recessed bay floors.
loft('01 | continuous armored hull with mapped belly',[
    (57,2,.65,.74,.79),(92,26,.50,.83,.94),(126,44,.34,.95,1.10),
    (163,52,.20,1.02,1.30),(201,81,.07,1.07,1.48),
    (242,86,-.04,1.10,1.54),(284,104,-.13,1.13,1.51),
    (312,113,-.22,1.15,1.39),(330,117,-.30,1.10,1.25),
    (350,162,-.38,1.13,1.17),(381,187,-.46,1.10,1.02),
    (415,211,-.50,1.00,.83),(445,218,-.52,.89,.74),
    (476,210,-.53,.84,.67),(513,201,-.52,.78,.62),
    (550,184,-.48,.76,.60),(588,160,-.37,.77,.64),
    (618,139,-.23,.75,.69),(637,110,-.08,.75,.79),
    (661,75,.03,.81,.93),(686,43,.12,.84,1.10),
    (732,20,.21,.82,1.14),(800,4,.40,.58,.72),
])

# Raised central dorsal spine and the layered tapered prow armor.
loft('02 | central dorsal service spine',[
    (304,21,.98,1.47,1.73),(337,30,.78,1.48,1.80),
    (391,31,.67,1.54,1.87),(451,29,.63,1.56,1.86),
    (510,27,.59,1.49,1.77),(566,27,.59,1.49,1.78),
    (616,24,.66,1.43,1.70),(659,18,.76,1.23,1.47),
    (704,10,.83,1.05,1.23),
])
prow=[(296,60),(304,84),(333,148),(343,174),(353,198),(378,225),
      (388,262),(384,286),(353,272),(321,239),(301,190)]
prow_height=lambda x,y:interpolate(y,[(60,.84),(150,1.37),(210,1.66),(286,1.67)])
for side,label in [(-1,'port'),(1,'starboard')]:
    slab('03 | '+label+' raised prow armor',prow,prow_height,
         lambda x,y:prow_height(x,y)-.10,side)

# Each shoulder is an actual U-shaped slab around an open aft-facing recess.
shoulder=[(354,300),(383,309),(407,338),(453,351),(480,384),(504,419),
          (511,450),(499,481),(484,512),(476,548),(453,592),(433,618),
          (411,621),(399,608),(400,585),(403,493),(346,493),(341,577),
          (323,605),(318,570),(319,485),(329,443),(332,382)]
deck_z=lambda x,y:interpolate(y,[(300,1.59),(372,1.74),(444,1.64),(520,1.51),(621,1.37)])
def deck_mapping(normal,center):
    if normal[2]>.28:return atlas,top_uv
    if normal[2]<-.30:return ventral,bottom_uv
    px=CX+abs(center[0])*S;py=CY-center[1]*S
    if 335<px<410 and 486<py<612:return graphite,top_uv
    return hull_mapping(normal,center)
for side,label in [(-1,'port'),(1,'starboard')]:
    slab('04 | '+label+' shoulder armor around recessed bay',shoulder,deck_z,.72,side,mode=deck_mapping,bevel=.085)
    # Distinct outer edge armor and a small cyan outboard marker.
    edge=[(482,397),(507,419),(516,445),(511,467),(499,483),(495,451)]
    slab('05 | '+label+' outer shoulder collar',edge,1.36,.32,side)
    px=CX+side*218
    beam('05 | '+label+' outer cyan running light',point(px,439,.72),point(px,453,.72),.018,.095,cyan)
    # The floor is below the surrounding real bay walls, with a small docked craft.
    bx=CX+side*79
    bounds=(bx-23,496,bx+23,594)
    bevel_box('06 | '+label+' recessed bay deck',bounds,.56,.64,.03,.10,mode='top_bottom')
    for dx in [-20,20]:
        beam('06 | '+label+' bay guide rail '+str(dx),point(bx+dx,512,.68),point(bx+dx,581,.68),.035,.045,brass)
    # A physical miniature shuttle follows the tiny silhouette in the reference.
    loft('06 | '+label+' docked shuttle',[
        (520,1,.66,.72,.83),(531,3.5,.66,.77,.92),
        (548,5.5,.66,.75,.86),(561,4,.66,.71,.78),
    ],center=bx,mode='top_bottom')
    for dx in [-4,4]:
        radial('06 | '+label+' shuttle engine '+str(dx),bx+dx,[(551,.036),(560,.052)],.70,mat=dark,segments=16)
        radial('06 | '+label+' shuttle cyan nozzle '+str(dx),bx+dx,[(560,.038),(560.6,.038)],.70,mat=cyan,segments=16)
    sockets.append(('socket.bay.'+label,point(bx,546,.70)))

# Raised command bridge; its side glazing uses its own station on the sheet.
def bridge_mapping(n,center):
    if n[2]>.32:return atlas,top_uv
    if n[2]<-.32:return ventral,bottom_uv
    if abs(n[0])>.5:
        def uv(v):
            t=clamp((CY-v[1]*S-211)/95)
            h=clamp((v[2]-1.45)/1.04)
            return (1140+t*114)/2048,(678-h*39)/2048
    else:
        def uv(v):
            t=clamp((v[0]*S+24)/48)
            h=clamp((v[2]-1.45)/1.04)
            return (1306+t*78)/2048,(178-h*50)/2048
    return atlas,uv
loft('07 | raised command bridge',[
    (212,12,1.48,1.72,1.92),(230,20,1.46,1.94,2.27),
    (251,23,1.44,2.14,2.47),(278,25,1.43,2.12,2.41),
    (299,19,1.41,1.87,2.10),(309,10,1.42,1.68,1.82),
],mode=bridge_mapping)

# Two independent twin-barrel turrets on raised beveled sockets.
def turret_mapping(pc):
    def mapping(n,center):
        if n[2]>.35:return atlas,top_uv
        if n[2]<-.35:return graphite,top_uv
        if abs(n[0])>.5:
            def uv(v):
                t=clamp((CY-v[1]*S-340)/77)
                h=clamp((v[2]-1.65)/.80)
                return (1305+t*92)/2048,(674-h*34)/2048
        else:
            def uv(v):
                t=clamp((CX+v[0]*S-pc+23)/46)
                h=clamp((v[2]-1.65)/.80)
                src=1147 if pc<CX else 1468
                return (src+t*80)/2048,(194-h*49)/2048
        return atlas,uv
    return mapping
for side,label in [(-1,'port'),(1,'starboard')]:
    pc=CX+side*140
    mode=turret_mapping(pc)
    bevel_box('turret.'+label+' | armored rotation pedestal',(pc-24,351,pc+24,417),1.64,1.93,.10,.19,mode=mode)
    bevel_box('turret.'+label+' | twin-gun housing',(pc-16,337,pc+16,395),1.86,2.39,.11,.19,mode=mode)
    for i,dx in enumerate([-5.5,5.5],1):
        gx=pc+dx
        prefix='turret.'+label+f'.barrel.{i}'
        radial(prefix+' | brass breech sleeve',gx,[(330,.10),(346,.11),(351,.09)],2.28,mat=brass,segments=24)
        radial(prefix+' | gunmetal barrel',gx,[(294,.066),(298,.082),(329,.079),(337,.084)],2.28,mat=graphite,cap=False,segments=24)
        ring(prefix+' | muzzle lip',gx,294,2.28,.070,.044,.042,silver)
        radial(prefix+' | recessed bore',gx,[(297,.043),(298,.043)],2.28,mat=dark,segments=24)
        sockets.append((f'socket.weapon.{label}.{i}',point(gx,292,2.28)))
    sockets.append(('socket.turret.'+label,point(pc,378,1.74)))

# Dedicated ventral geometry carries the BOTTOM artwork, including all radiators.
def ventral_mapping(n,center):
    if abs(n[2])>.25:return ventral,bottom_uv
    return atlas,side_uv
bevel_box('08 | ventral keel radiator enclosure',(279,344,310,523),-.60,-.33,.055,.10,mode=ventral_mapping)
for side,label in [(-1,'port'),(1,'starboard')]:
    px=CX+side*106
    bevel_box('08 | '+label+' ventral radiator hatch',(px-14,536,px+14,594),-.52,-.30,.035,.075,mode=ventral_mapping)

# Four stepped cylindrical drives and truly recessed textured exhaust apertures.
engines=[('port.outer',CX-123,True,1168,497,21),
         ('port.inner',CX-51,False,1274,495,30),
         ('starboard.inner',CX+51,False,1411,495,30),
         ('starboard.outer',CX+123,True,1518,497,21)]
for label,px,outer,srcx,srcy,srcr in engines:
    prefix='engine.'+label
    radius=.61 if outer else .80
    end=750 if outer else 781
    z=.82
    def casing_mapping(n,center,radius=radius,px=px,end=end):
        if n[2]>.30:return atlas,top_uv
        if n[2]<-.30:return ventral,bottom_uv
        if abs(n[1])>.75:return graphite,top_uv
        def uv(v):
            t=clamp((CY-v[1]*S-620)/(end-620))
            h=clamp((v[2]-(z-radius))/(2*radius))
            return (1643+t*91)/2048,(761-h*86)/2048
        return atlas,uv
    radial(prefix+' | armored nacelle',px,[(619,radius*.48),(641,radius*.75),(669,radius),
           (end-34,radius),(end-19,radius*.85),(end-6,radius*.76)],z,mode=casing_mapping,cap=False)
    ring(prefix+' | brass coupling',px,end-30,z,radius*1.008,radius*.94,.085,brass)
    ring(prefix+' | titanium exhaust rim',px,end,z,radius*.80,radius*.665,.10,silver)
    ring(prefix+' | dark inner nozzle',px,end-6,z,radius*.671,radius*.54,.34,dark)
    ring(prefix+' | inner retaining collar',px,end-8,z,radius*.55,radius*.49,.05,brass)
    def aperture_mapping(n,center,px=px,srcx=srcx,srcy=srcy,srcr=srcr,radius=radius):
        if abs(n[1])<.9:return dark,top_uv
        def uv(v):
            scale=srcr/(radius*.535)
            return (srcx+(CX+v[0]*S-px)/S*scale)/2048,(srcy-(v[2]-z)*scale)/2048
        return atlas,uv
    radial(prefix+' | recessed cyan reactor aperture',px,[(end-9.4,radius*.533),(end-9,radius*.533)],z,mode=aperture_mapping,segments=64)
    sockets.append(('socket.'+prefix,point(px,end+3,z)))

export()
