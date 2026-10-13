"""Small deterministic mesh and glTF pipeline. Requires NumPy; no DCC application."""
from pathlib import Path
import json
import math
import struct
import numpy as np

ROOT = Path(__file__).resolve().parent
S = 50.0
CX, CY = 257.0, 430.0
parts = []
sockets = []
materials = []

def point(px, py, z):
    return ((px-CX)/S, (CY-py)/S, z)

def mirrored(pixels, side):
    return [(CX+side*(x-CX), y) for x, y in pixels]

def interpolate(x, rows):
    for (a, va), (b, vb) in zip(rows, rows[1:]):
        if x <= b:
            return va+(vb-va)*max(0, (x-a)/(b-a))
    return rows[-1][1]

def solid(name, color, metallic=.35, roughness=.45, emission=0):
    mat = {"name":name,"pbrMetallicRoughness":{"baseColorFactor":[*color,1],"metallicFactor":metallic,"roughnessFactor":roughness}}
    if emission:
        mat["emissiveFactor"] = [min(1,c*emission) for c in color]
    materials.append(mat)
    return len(materials)-1

atlas = solid("Reference artwork | titanium, brass, panels and canopy", (1,1,1),1,1)
materials[atlas]["pbrMetallicRoughness"].update({"baseColorTexture":{"index":0},"metallicRoughnessTexture":{"index":1}})
materials[atlas].update({"emissiveTexture":{"index":2},"emissiveFactor":[1,1,1]})
ventral = solid("Ventral armor | dedicated bottom-view reference", (1,1,1),1,1)
materials[ventral]["pbrMetallicRoughness"].update({"baseColorTexture":{"index":0},"metallicRoughnessTexture":{"index":1}})
materials[ventral].update({"emissiveTexture":{"index":2},"emissiveFactor":[1,1,1]})
graphite = solid("Structural frames | gunmetal", (.025,.030,.037),.42,.55)
dark = solid("Structural rails | dark alloy", (.010,.014,.019),.40,.49)
silver = solid("Machined titanium edges", (.22,.25,.29),.55,.40)
brass = solid("Warm brass collars", (.52,.30,.025),.60,.40)
cyan = solid("Cyan luminous apertures", (.003,.15,.30),.1,.4,3.0)
glass = solid("Canopy edge seal", (.035,.055,.07),.25,.24)

def top_uv(v):
    return ((CX+v[0]*S)/2048,(CY-v[1]*S)/2048)

def bottom_uv(v):
    py=CY-v[1]*S
    py=interpolate(py,[(54,24),(80,53),(105,79),(142,123),(205,204),(230,225),(800,801)])
    return ((739-v[0]*S*.96)/2048,py/2048)

def side_uv(v):
    py=CY-v[1]*S
    x=interpolate(py,[(54,929),(83,965),(146,1055),(220,1127),(280,1151),
                     (425,1348),(453,1380),(595,1580),(650,1630),(755,1710),(800,1740)])
    return (x/2048,(808-v[2]*61)/2048)

def front_uv(v):
    return ((1363-v[0]*76.5)/2048,(318-v[2]*66)/2048)

def back_uv(v):
    return ((1363+v[0]*76.5)/2048,(578-v[2]*66)/2048)

def hull_mapping(normal, center=None):
    if normal[2]<-.30:return ventral,bottom_uv
    if normal[2]>.30:return atlas,top_uv
    if abs(normal[0])>=abs(normal[1]):return atlas,side_uv
    return atlas,front_uv if normal[1]>0 else back_uv

def panel_uv(bounds, source):
    """Map a horizontal armor face to a chosen native reference panel."""
    x0,y0,x1,y1=bounds
    u0,v0,u1,v1=source
    def uv(v):
        x=(CX+v[0]*S-x0)/(x1-x0)
        y=(CY-v[1]*S-y0)/(y1-y0)
        return ((u0+x*(u1-u0))/2048,(v0+y*(v1-v0))/2048)
    return uv

def unit(a):
    length=np.linalg.norm(a)
    return a/length if length>1e-10 else np.array([0.,0.,1.])

def polygon_normal(vertices):
    return unit(np.sum(np.cross(vertices,np.roll(vertices,-1,axis=0)),axis=0))

def triangulate(vertices):
    """Ear clipping for the concave swept-wing contours; preserves winding."""
    n=len(vertices)
    if n==3:
        return [(0,1,2)]
    norm=polygon_normal(vertices)
    axis=int(np.argmax(np.abs(norm)))
    xy=np.delete(vertices,axis,axis=1)
    area=np.sum(xy[:,0]*np.roll(xy[:,1],-1)-np.roll(xy[:,0],-1)*xy[:,1])
    direction=1 if area>=0 else -1
    indices=list(range(n))
    result=[]
    cross=lambda a,b:a[0]*b[1]-a[1]*b[0]
    def inside(p,a,b,c):
        return (direction*cross(b-a,p-a)>1e-10 and direction*cross(c-b,p-b)>1e-10 and direction*cross(a-c,p-c)>1e-10)
    while len(indices)>3:
        found=False
        for k,b in enumerate(indices):
            a,c=indices[k-1],indices[(k+1)%len(indices)]
            if direction*cross(xy[b]-xy[a],xy[c]-xy[b])<1e-12:
                continue
            if any(inside(xy[j],xy[a],xy[b],xy[c]) for j in indices if j not in (a,b,c)):
                continue
            result.append((a,b,c))
            indices.pop(k)
            found=True
            break
        if not found:
            # Near-collinear contours can have zero-area ears; remove those first.
            areas=[abs(cross(xy[indices[k]]-xy[indices[k-1]],xy[indices[(k+1)%len(indices)]]-xy[indices[k]])) for k in range(len(indices))]
            k=int(np.argmin(areas))
            if areas[k]>1e-7:
                raise ValueError("Could not triangulate polygon")
            indices.pop(k)
    if len(indices)==3 and abs(cross(xy[indices[1]]-xy[indices[0]],xy[indices[2]]-xy[indices[0]]))>1e-12:
        result.append(tuple(indices))
    return result

def mesh(name, vertices, faces, mode="auto", mat=atlas, bevel=0, smooth=False):
    vertices=np.asarray(vertices,dtype=np.float64)
    faces=[list(f) for f in faces]
    # An outward-facing closed hull has positive signed volume.
    volume=sum(np.dot(vertices[f[0]],np.cross(vertices[f[j]],vertices[f[j+1]])) for f in faces for j in range(1,len(f)-1))/6
    if volume<0:
        faces=[list(reversed(f)) for f in faces]
    normals=[polygon_normal(vertices[f]) for f in faces]
    adjoining=[[] for _ in vertices]
    for fi,f in enumerate(faces):
        for v in f:
            adjoining[v].append(fi)
    groups={}
    for fi,face in enumerate(faces):
        normal=normals[fi]
        chosen=mat
        projection=top_uv
        if mat==atlas:
            if callable(mode):
                chosen, projection = mode(normal,vertices[face].mean(axis=0))
            elif mode=="auto":
                chosen,projection=hull_mapping(normal)
            elif mode=="fin":
                if abs(normal[2])<.4:
                    chosen=dark
            elif mode=="side":
                projection=side_uv
            elif mode=="top_bottom" and normal[2]<-.45:
                chosen=ventral
                projection=bottom_uv
        group=groups.setdefault(chosen,{"position":[],"normal":[],"uv":[],"indices":[]})
        start=len(group["position"])
        for vi in face:
            n=normal
            if smooth:
                n=unit(sum((normals[k] for k in adjoining[vi] if np.dot(normals[k],normal)>.65),start=np.zeros(3)))
            group["position"].append(vertices[vi])
            group["normal"].append(n)
            group["uv"].append(projection(vertices[vi]))
        for triangle in triangulate(vertices[face]):
            group["indices"].extend(start+k for k in triangle)
    parts.append({"name":name,"groups":groups})
    return parts[-1]

def export():
    # Keep authored components in the OBJ and deterministic build source, while
    # consolidating static GLB geometry by material for fleet rendering.
    consolidated={}
    for part in parts:
        for material,g in part['groups'].items():
            merged=consolidated.setdefault(material,{'position':[],'normal':[],'uv':[],'indices':[],'components':[]})
            offset=len(merged['position'])
            merged['components'].append({'name':part['name'],'indexStart':len(merged['indices']),'indexCount':len(g['indices'])})
            for attribute in ['position','normal','uv']:merged[attribute].extend(g[attribute])
            merged['indices'].extend(offset+i for i in g['indices'])
    render_parts=[]
    for material,g in consolidated.items():
        # Merge only identical position/normal/UV tuples, preserving hard edges.
        vertices=np.column_stack([g['position'],g['normal'],g['uv']]).astype('<f4')
        unique,inverse=np.unique(vertices,axis=0,return_inverse=True)
        g.update(position=unique[:,:3],normal=unique[:,3:6],uv=unique[:,6:8],indices=inverse[g['indices']].tolist())
        render_parts.append({'name':materials[material]['name'],'groups':{material:g},'components':g['components']})
    binary=bytearray()
    doc={"asset":{"version":"2.0","generator":"Stellar Syndicates reference-to-mesh pipeline (Python/NumPy)"},"scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"name":"HEAVY FREIGHTER","children":[],"extras":{"source":"heavy-freighter-reference-v1/heavy-freighter-reference.png","textureMethod":"Separate UV projections from the top, bottom, side, front and back reference views","note":"Editable reconstruction of the illustrated five-view reference; no Blender"}}],"meshes":[],"materials":materials,"buffers":[{}],"bufferViews":[],"accessors":[],"images":[],"samplers":[{"magFilter":9729,"minFilter":9987,"wrapS":33071,"wrapT":33071}],"textures":[]}
    def data_view(raw,target=None):
        while len(binary)%4: binary.append(0)
        start=len(binary)
        binary.extend(raw)
        view={"buffer":0,"byteOffset":start,"byteLength":len(raw)}
        if target: view["target"]=target
        doc["bufferViews"].append(view)
        return len(doc["bufferViews"])-1
    def accessor(data,component,kind,target,minmax=False):
        array=np.asarray(data,dtype={5126:'<f4',5125:'<u4',5123:'<u2'}[component])
        acc={"bufferView":data_view(array.tobytes(),target),"componentType":component,"count":len(array),"type":kind}
        if minmax:
            acc["min"]=array.min(axis=0).tolist()
            acc["max"]=array.max(axis=0).tolist()
        doc["accessors"].append(acc)
        return len(doc["accessors"])-1
    for filename in ["heavy-freighter-basecolor.png","heavy-freighter-metallic-roughness.png","heavy-freighter-emissive.png"]:
        doc["images"].append({"name":filename,"mimeType":"image/png","bufferView":data_view((ROOT/"textures"/filename).read_bytes())})
        doc["textures"].append({"sampler":0,"source":len(doc["images"])-1})
    points=[]
    triangle_count=0
    for part in render_parts:
        primitives=[]
        for material,group in part["groups"].items():
            positions=np.array(group["position"])
            normals=np.array(group["normal"])
            # glTF uses Y-up: our +Y nose becomes glTF -Z.
            positions=positions[:,[0,2,1]]*np.array([1,1,-1])
            normals=normals[:,[0,2,1]]*np.array([1,1,-1])
            points.extend(positions)
            triangle_count+=len(group["indices"])//3
            index_type=5123 if len(positions)<=65535 else 5125
            primitives.append({"attributes":{"POSITION":accessor(positions,5126,"VEC3",34962,True),"NORMAL":accessor(normals,5126,"VEC3",34962),"TEXCOORD_0":accessor(group["uv"],5126,"VEC2",34962)},"indices":accessor(group["indices"],index_type,"SCALAR",34963),"material":material})
        doc["meshes"].append({"name":part["name"],"primitives":primitives,"extras":{"sourceComponents":part['components']}})
        doc["nodes"].append({"name":part["name"],"mesh":len(doc["meshes"])-1})
        doc["nodes"][0]["children"].append(len(doc["nodes"])-1)
    for name,loc in sockets:
        doc["nodes"].append({"name":name,"translation":[loc[0],loc[2],-loc[1]]})
        doc["nodes"][0]["children"].append(len(doc["nodes"])-1)
    doc["buffers"][0]["byteLength"]=len(binary)
    json_bytes=json.dumps(doc,separators=(",",":")).encode()
    while len(json_bytes)%4:json_bytes+=b" "
    while len(binary)%4:binary.append(0)
    length=12+8+len(json_bytes)+8+len(binary)
    glb=struct.pack("<4sII",b"glTF",2,length)+struct.pack("<I4s",len(json_bytes),b"JSON")+json_bytes+struct.pack("<I4s",len(binary),b"BIN\0")+binary
    (ROOT/"heavy-freighter.glb").write_bytes(glb)
    points=np.array(points)
    stats={"meshes":len(render_parts),"renderSections":len(render_parts),"sourceComponents":len(parts),"triangles":triangle_count,"exportedVertices":len(points),"dimensionsXYZ":np.round(np.ptp(points,axis=0),4).tolist(),"textureResolution":[2048,2048],"embeddedTextures":3,"axes":"Y up, -Z forward","source":"../../heavy-freighter-reference-v1/heavy-freighter-reference.png","generator":"Python/NumPy, no Blender","optimization":"Static parts combined by material; exact vertex deduplication; 16-bit indices where possible"}
    (ROOT/"model-info.json").write_text(json.dumps(stats,indent=2)+"\n")
    print(json.dumps(stats,indent=2))

    # OBJ/MTL is an additional broadly compatible editable interchange export.
    lines=["# Heavy Freighter, Y-up, textures from the approved reference","mtllib heavy-freighter.mtl"]
    offset=1
    for part in parts:
        lines.append("o "+part["name"].replace(" ","_"))
        for material,g in part["groups"].items():
            lines.append("usemtl material_"+str(material))
            for x,y,z in g["position"]:lines.append(f"v {x:.6f} {z:.6f} {-y:.6f}")
            for u,v in g["uv"]:lines.append(f"vt {u:.7f} {1-v:.7f}")
            for x,y,z in g["normal"]:lines.append(f"vn {x:.7f} {z:.7f} {-y:.7f}")
            indices=g["indices"]
            for i in range(0,len(indices),3):
                lines.append("f "+" ".join(f"{k+offset}/{k+offset}/{k+offset}" for k in indices[i:i+3]))
            offset+=len(g["position"])
    (ROOT/"heavy-freighter.obj").write_text("\n".join(lines)+"\n")
    mtl=[]
    for i,mat in enumerate(materials):
        pbr=mat["pbrMetallicRoughness"]
        mtl.extend([f"newmtl material_{i}","Kd "+" ".join(str(c) for c in pbr["baseColorFactor"][:3]),"Ks .25 .25 .25","Ns 48"])
        if "baseColorTexture" in pbr:mtl.extend(["map_Kd textures/heavy-freighter-basecolor.png","map_Ke textures/heavy-freighter-emissive.png"])
        mtl.append("")
    (ROOT/"heavy-freighter.mtl").write_text("\n".join(mtl))
