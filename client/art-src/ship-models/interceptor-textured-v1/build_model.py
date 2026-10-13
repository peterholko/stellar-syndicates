"""Build a UV-textured Interceptor GLB and OBJ without Blender.

Run: python3 build_model.py
"""
from mesh_core import *

def slab(name, outline, top, bottom, side=1, mat=atlas, mode="top_bottom", bevel=.008):
    outline = mirrored(outline, side)
    if sum(outline[i][0]*outline[(i+1)%len(outline)][1] - outline[(i+1)%len(outline)][0]*outline[i][1] for i in range(len(outline))) > 0:
        outline.reverse()
    n = len(outline)
    ztop = lambda x, y: top(x,y) if callable(top) else top
    zlow = lambda x, y: bottom(x,y) if callable(bottom) else bottom
    verts = [point(x,y,zlow(x,y)) for x,y in outline] + [point(x,y,ztop(x,y)) for x,y in outline]
    faces = [tuple(reversed(range(n))), tuple(range(n, n*2))]
    faces += [(i, (i+1)%n, (i+1)%n+n, i+n) for i in range(n)]
    return mesh(name, verts, faces, mode, mat, bevel)

def loft(name, sections, mode="auto", mat=atlas):
    # Each station is y-pixel, half-width-pixels, bottom Z, shoulder Z, ridge Z.
    verts=[]
    for py, width, low, shoulder, ridge in sections:
        for xx, zz in [(0,ridge), (.67*width,ridge-.06), (width,shoulder), (.81*width,min(low+.10,shoulder-.035)), (0,low), (-.81*width,min(low+.10,shoulder-.035)), (-width,shoulder), (-.67*width,ridge-.06)]:
            verts.append(point(CX+xx, py, zz))
    faces=[tuple(reversed(range(8)))]
    for k in range(len(sections)-1):
        for j in range(8):
            faces.append((k*8+j,k*8+(j+1)%8,(k+1)*8+(j+1)%8,(k+1)*8+j))
    faces.append(tuple(range(len(verts)-8,len(verts))))
    return mesh(name,verts,faces,mode,mat,.009)

# Main body: faceted sections form a real tapered hull rather than a flat cutout.
hull_sections=[
    (78,6,.57,.66,.73),(102,11,.47,.68,.80),(175,27,.30,.76,1.08),
    (236,44,.20,.86,1.32),(280,54,.12,1.01,1.47),(338,65,.02,1.08,1.53),
    (414,72,-.03,1.15,1.64),(465,82,-.05,1.20,1.93),
    (506,74,-.06,1.19,1.94),(556,57,-.04,1.05,1.83),
    (620,44,.02,.92,1.63),(703,32,.10,.72,1.40),
    (782,20,.19,.55,1.13),(833,7,.31,.46,.81),
]

def hull_bottom_uv(v):
    """Use the aft hull's armor and access panels, excluding the upper canopy."""
    py=CY-v[1]*S
    width=interpolate(py,[(row[0],row[1]) for row in hull_sections])
    # Follow the tapered silver center plate at its native atlas position.
    source_y=460+(py-78)/(833-78)*(808-460)
    source_width=interpolate(source_y,[(460,15),(485,20),(515,18),(540,13),
                                     (574,23),(650,21),(710,12),(795,8),(808,6)])
    source_x=CX+v[0]*S/width*source_width
    return source_x/1536,source_y/1024

def hull_mapping(normal):
    if normal[2]<-.35:return ventral,hull_bottom_uv
    if abs(normal[0])>.68 and abs(normal[2])<.28:return atlas,side_uv
    if normal[1]>.75 and normal[2]<.45:return atlas,front_uv
    return atlas,top_uv

loft("01 Fuselage | continuous faceted pressure hull",hull_sections,mode=hull_mapping)

# Cockpit glazing is a separate vaulted volume mapped to its original black glass.
loft("02 Cockpit | smoked canopy and painted frames", [
    (251,15,1.39,1.45,1.55),(273,25,1.42,1.55,1.85),
    (316,35,1.47,1.64,2.16),(362,38,1.52,1.70,2.27),
    (400,32,1.57,1.73,2.20),(436,26,1.67,1.75,2.04),
    (456,20,1.73,1.78,1.87),
], mode="top", mat=atlas)

wing=[(427,431),(458,431),(496,481),(552,541),(649,639),(659,650),
      (662,749),(576,725),(500,700),(470,660),(451,609),(421,585),(409,520)]
wing_z=lambda x,y: .65+.30*(1-min(abs(x-CX)/316,1))
wing_bottom=lambda x,y: wing_z(x,y)-.19
tip=[(660,595),(666,592),(674,618),(679,669),(681,747),(671,773),(656,755),(651,684)]
for side,label in [(1,"Starboard"),(-1,"Port")]:
    slab(f"03 {label} | swept split-delta wing", wing, wing_z, wing_bottom, side)
    slab(f"04 {label} | raised root armor", [(416,430),(446,423),(461,463),(460,531),(449,579),(427,601),(408,565)],
         lambda x,y: interpolate(y,[(423,1.35),(482,1.42),(601,1.10)]), .65, side, mode="auto")
    slab(f"05 {label} | vertical wingtip endplate",tip,
         lambda x,y:interpolate(y,[(592,.62),(626,1.12),(672,1.17),(745,.91),(774,.54)]),.27,side,mode="fin")

def radial(name, center_px, stations, zcenter, mode="top", mat=atlas, segments=40, cap=True, smooth=True):
    # Stations are y-pixel, radius; engine cylinders follow the traced plan silhouette.
    verts=[]
    for py, radius in stations:
        for i in range(segments):
            theta=2*math.pi*i/segments
            verts.append(point(center_px+math.cos(theta)*radius*S,py,zcenter+math.sin(theta)*radius))
    faces=[]
    for k in range(len(stations)-1):
        for i in range(segments):
            faces.append((k*segments+i,k*segments+(i+1)%segments,(k+1)*segments+(i+1)%segments,(k+1)*segments+i))
    if cap:
        faces.extend([tuple(reversed(range(segments))),tuple(range(len(verts)-segments,len(verts)))])
    return mesh(name,verts,faces,mode,mat,0,smooth)

def ring(name, px, py, z, outer, inner, depth, mat=dark):
    # Closed annular shell with a real open bore.
    rows=[(py-depth*S/2,outer),(py+depth*S/2,outer),(py+depth*S/2,inner),(py-depth*S/2,inner),(py-depth*S/2,outer)]
    return radial(name,px,rows,z,mat=mat,cap=False,segments=48)

for side,label in [(1,"Starboard"),(-1,"Port")]:
    ex=CX+side*94
    radial(f"07 {label} | sculpted engine fairing",ex,[(580,.20),(612,.44),(650,.70),(697,.87),(753,.91),(794,.90),(818,.80)],.79,mode="top")
    radial(f"08 {label} | nozzle jacket",ex,[(815,.80),(830,.72),(840,.64),(868,.62),(881,.56)],.79,mode="top",cap=False)
    ring(f"09 {label} | dark exhaust lip",ex,881,.79,.56,.465,.13)
    ring(f"10 {label} | recessed nozzle wall",ex,873,.79,.467,.385,.40,mat=dark)
    ring(f"11 {label} | inner titanium retaining ring",ex,862,.79,.389,.325,.09,mat=silver)
    radial(f"12 {label} | recessed cyan reactor aperture",ex,[(873,.321),(874,.321)],.79,mat=cyan,segments=48)
    ring(f"13 {label} | reactor aperture bezel",ex,876,.79,.327,.295,.04,mat=brass)
    # Forward gun units use the same exact top texture, with a modeled muzzle bore.
    gx=CX+side*105
    radial(f"14 {label} | forward weapon housing",gx,[(292,.10),(299,.17),(325,.20),(363,.21),(385,.22),(408,.25),(434,.23)],1.13,segments=20)
    ring(f"15 {label} | weapon muzzle",gx,292,1.13,.102,.062,.06,mat=brass)
    radial(f"16 {label} | recessed weapon bore",gx,[(294,.063),(299,.063)],1.13,mat=dark,segments=20)

# Wingtip lights are real narrow inserts on the outboard faces, continuing the atlas light.
for side,label in [(1,"Starboard"),(-1,"Port")]:
    a=CX+side*328.5
    b=CX+side*331.2
    light=[point(a,662,.93),point(b,742,.74),point(b,742,.66),point(a,662,.85)]
    mesh(f"17 {label} | outboard cyan light insert",light,[(0,1,2,3),(3,2,1,0)],mat=cyan)

export()
