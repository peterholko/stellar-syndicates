"""Build the ship named in model-spec.json from its five-view reference.

Authored parts remain editable in OBJ. Static glTF parts merge by material;
turret assemblies retain named pivots and weapon sockets for the LOD exporter.
"""
from geometry import *

def box(name,bounds,low=.3,high=2.0,**kw):
    return bevel_box(name,bounds,low,high,**kw)

def zbody(name,px,py,rows,mat=graphite,segments=32,mode='auto'):
    v=[]
    for r,z in rows:
        v.extend(point(px+math.cos(i*math.tau/segments)*r*S,
                       py+math.sin(i*math.tau/segments)*r*S,z) for i in range(segments))
    f=[tuple(reversed(range(segments)))]
    for k in range(len(rows)-1):
        f.extend((k*segments+i,k*segments+(i+1)%segments,(k+1)*segments+(i+1)%segments,(k+1)*segments+i) for i in range(segments))
    f.append(tuple(range(len(v)-segments,len(v))))
    return mesh(name,v,f,mat=mat,mode=mode,smooth=True)

def navigation(px,py,z,name='center'):
    box('light.'+name+' | cyan running light',(px-2,py-6,px+2,py+6),z,z+.025,bevel=.005,mat=cyan)

def antenna(px,py,z,height=.7,name='bridge'):
    zbody('sensor.'+name+' | mast',px,py,[(.055,z),(.045,z+height*.70),(.016,z+height)],silver,24)

def bridge(bounds,base,height=1.0):
    x0,y0,x1,y1=bounds;px=(x0+x1)/2
    loft('bridge | faceted command cabin',[(y0,(x1-x0)*.20,base,base+height*.55,base+height*.70),
         (y0+(y1-y0)*.23,(x1-x0)/2,base,base+height*.75,base+height),
         (y1-(y1-y0)*.14,(x1-x0)*.44,base,base+height*.62,base+height*.83),
         (y1,(x1-x0)*.28,base,base+height*.35,base+height*.55)],center=px)
    sockets.append(('socket.bridge',point(px,(y0+y1)/2,base+height)))

def engine(name,px,ystart,yend,r,z,backcenter,backradius,upper=True):
    length=yend-ystart
    shellmode=lambda n,c:(graphite,top_uv) if abs(n[1])>.7 else hull_mapping(n,c)
    radial('enginecase.'+name+' | pressure casing',px,
           [(ystart,r*.67),(ystart+length*.13,r*.87),(ystart+length*.36,r),
            (ystart+length*.66,r*.96),(ystart+length*.73,r*.85)],z,
           segments=32,mode=shellmode)
    # Reference-projected armor covers both sides of the pressure housing.
    if upper:
        bounds=(px-r*S*.72,ystart+length*.03,px+r*S*.72,ystart+length*.52)
        box('armor.engine.'+name+' | upper cowl',bounds,z+r*.72,z+r*1.04,bevel=.055)
        box('keel.engine.'+name+' | lower cowl',bounds,z-r*1.04,z-r*.72,bevel=.055)
    rear=ystart+length*.61
    radial('engine.'+name+' | hollow nozzle',px,
       [(rear,r*.92),(ystart+length*.70,r*.88),(ystart+length*.81,r*.77),
        (yend-3,r*.69),(yend,r*.67),(yend,r*.56),(yend-4,r*.55),
        (ystart+length*.80,r*.63),(rear,r*.73),(rear,r*.92)],z,
        mat=graphite,cap=False,segments=64)
    ring('engine.'+name+' | machined lip',px,yend-1,z,r*.69,r*.57,.045,silver)
    ring('engine.'+name+' | brass annulus',px,yend-6,z,r*.58,r*.52,.04,brass)
    for i,t in enumerate((.69,.79,.89)):
        rr=r*(.9-(t-.69)*.91)
        ring('engine.'+name+f'.{i} | nozzle band',px,ystart+length*t,z,rr,rr-.045,.045,dark)
    wx=(px-CX)/S
    def uv(v):return ((backcenter[0]+(v[0]-wx)*backradius/(r*.54))/2048,
                      (backcenter[1]-(v[2]-z)*backradius/(r*.54))/2048)
    radial('engine.'+name+' | recessed exhaust face',px,[(yend-9,r*.54),(yend-8,r*.54)],z,
           mode=lambda n,c:(atlas,uv) if abs(n[1])>.7 else (graphite,top_uv),segments=64)
    for j in range(8):
        a=j*math.tau/8
        beam('engine.'+name+f'.rib.{j}',point(px+math.cos(a)*r*.9*S,rear+4,z+math.sin(a)*r*.9),
             point(px+math.cos(a)*r*.72*S,yend-5,z+math.sin(a)*r*.72),.025,.032,silver)
    sockets.append(('socket.engine.'+name,point(px,yend+1,z)))

def turret(label,index,px,py,deck,r=.47,barrel_length=42,yaw=0):
    prefix=f'turret.{label}.{index}';start=len(parts)
    zbody(prefix+' | rotating pedestal',px,py,[(r*.75,deck-.04),(r,deck+.045),(r*.95,deck+.14),(r*.76,deck+.19)],segments=32)
    zbody(prefix+' | brass bearing',px,py,[(r*1.01,deck+.055),(r*1.01,deck+.09)],brass,32)
    width=r*S*.78
    box(prefix+' | armored turret housing',(px-width,py-width*.9,px+width,py+width*.75),deck+.14,deck+.55,
        bevel=.065,corner=.13,mode=lambda n,c:(atlas,top_uv) if n[2]>.35 else (graphite,top_uv))
    muzzle=[]
    for j,dx in enumerate((-r*S*.32,r*S*.32),1):
        bz=deck+.43;bx=px+dx
        radial(prefix+f' | barrel {j}',bx,[(py-barrel_length,r*.10),(py-barrel_length+5,r*.12),
                (py-12,r*.13),(py-3,r*.20)],bz,mat=graphite,segments=32)
        ring(prefix+f' | collar {j}',bx,py-12,bz,r*.21,r*.14,.07,brass)
        ring(prefix+f' | muzzle {j}',bx,py-barrel_length,bz,r*.14,r*.065,.07,silver)
        muzzle.append((f'socket.weapon.{label}.{index}.{j}',point(bx,py-barrel_length-1,bz)))
    pivot=np.array(point(px,py,deck+.07))
    c,s=math.cos(yaw),math.sin(yaw);rot=np.array([[c,-s,0],[s,c,0],[0,0,1]])
    for part in parts[start:]:
        for material,g in part['groups'].items():
            g['position']=[rot@(np.array(v)-pivot)+pivot for v in g['position']]
            g['normal']=[rot@np.array(v) for v in g['normal']]
            if material==atlas:g['uv']=[top_uv(v) for v in g['position']]
    sockets.extend((n,rot@(np.array(v)-pivot)+pivot) for n,v in muzzle)
    sockets.append((f'socket.turret.{label}.{index}',pivot))

def habitat(name,px,py,rx,ry,z,above,below):
    """Ellipsoidal glazed dome and solid armored bowl, with unique pole vertices."""
    sectors=64;bands=20
    v=[point(px,py,z+above)]
    for k in range(1,bands):
        a=math.pi*k/bands
        h=(above if math.cos(a)>=0 else below)*math.cos(a)
        v.extend(point(px+rx*math.sin(a)*math.cos(j*math.tau/sectors),
                       py+ry*math.sin(a)*math.sin(j*math.tau/sectors),z+h) for j in range(sectors))
    bottom=len(v);v.append(point(px,py,z-below))
    f=[(0,1+j,1+(j+1)%sectors) for j in range(sectors)]
    for k in range(bands-2):
        a=1+k*sectors;b=a+sectors
        f.extend((a+j,b+j,b+(j+1)%sectors,a+(j+1)%sectors) for j in range(sectors))
    a=1+(bands-2)*sectors
    f.extend((a+j,bottom,a+(j+1)%sectors) for j in range(sectors))
    mesh('habitat.'+name+' | glazed dome and ventral bowl',v,f,smooth=True,
         mode=lambda n,c:(ventral,bottom_uv) if c[2]<z else (atlas,top_uv))
    # A structural equatorial band follows the actual dome rather than a flat disc.
    ellipse_ring('habitat.'+name+' | brass equatorial seal',px,py,rx/S,ry/S,.055,z,brass,64,8)

def ellipse_ring(name,px,py,rx,ry,tube,z,mat=atlas,around=72,cross=12):
    v=[]
    # Half-step sample avoids a quad spanning the projection's silhouette.
    for i in range(around):
        a=i*math.tau/around
        for j in range(cross):
            b=j*math.tau/cross
            v.append(point(px+(rx+tube*math.cos(b))*math.cos(a)*S,
                           py+(ry+tube*math.cos(b))*math.sin(a)*S,z+tube*math.sin(b)))
    f=[(i*cross+j,((i+1)%around)*cross+j,((i+1)%around)*cross+(j+1)%cross,i*cross+(j+1)%cross)
       for i in range(around) for j in range(cross)]
    mesh(name,v,f,mat=mat,smooth=True,
         mode=lambda n,c:(ventral,bottom_uv) if c[2]<z else (atlas,top_uv))

def cargo_box(label,index,bounds,low,high):
    prefix=f'cargo.{label}.{index}';x0,y0,x1,y1=bounds
    box(prefix+' | closed container',bounds,low,high,bevel=.14,corner=.28)
    # Raised hatch lips, corner latches and a central restraint wrap the box.
    inset=8
    box(prefix+' | upper loading panel',(x0+inset,y0+inset,x1-inset,y1-inset),high-.05,high+.10,bevel=.035)
    box(prefix+' | ventral access panel',(x0+inset,y0+inset,x1-inset,y1-inset),low-.09,low+.04,bevel=.03)
    cx=(x0+x1)/2
    box(prefix+' | restraint top',(cx-4,y0+2,cx+4,y1-2),high+.03,high+.15,bevel=.025,mat=dark)
    for py in (y0+4,y1-4):
        box(prefix+' | brass corner latch',(cx-4.5,py-3,cx+4.5,py+3),high+.13,high+.21,bevel=.02,mat=brass)

def truss(name,x0,x1,y0,y1,z,depth=.8):
    # Open X frames leave real negative space between modules and the spine.
    for i,(a,b) in enumerate([((x0,y0),(x1,y0)),((x0,y1),(x1,y1)),
                              ((x0,y0),(x1,y1)),((x0,y1),(x1,y0))]):
        beam('structure.'+name+f'.{i}',point(*a,z),point(*b,z),.11,.14,silver if i>1 else graphite)
    for x in (x0,x1):
        box('structure.'+name+' | gold docking clamp',(x-2.8,y0-2,x+2.8,y1+2),z-depth/2,z+depth/2,mat=brass,bevel=.028)

from designs import build
build(SPEC['id'],globals())
sockets.append(('socket.reactor',point(CX,CY+180,1.1)))
assert sum(n.startswith('socket.engine.') for n,_ in sockets)==SPEC['engines']
assert sum(n.startswith('socket.turret.') for n,_ in sockets)==SPEC['turrets']
export()
