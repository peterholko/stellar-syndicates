"""Geometry construction helpers for the Small Freighter."""
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
    return radial(name,px,rows,z,mat=mat,cap=False,segments=32)
