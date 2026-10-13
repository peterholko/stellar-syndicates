"""Small deterministic mesh and glTF pipeline. Requires NumPy; no DCC application."""
from pathlib import Path
import json
import math
import struct
import numpy as np

ROOT = Path(__file__).resolve().parent
S = 60.0
CX, CY = 351.0, 484.0
parts = []
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
ventral = solid("Ventral armor | mapped titanium panels and service detail", (.88,.91,.94),1,1)
materials[ventral]["pbrMetallicRoughness"].update({"baseColorTexture":{"index":0},"metallicRoughnessTexture":{"index":1}})
materials[ventral].update({"emissiveTexture":{"index":2},"emissiveFactor":[1,1,1]})
graphite = solid("Structural gunmetal", (.10,.12,.135),.50,.48)
dark = solid("Recesses | dark structural alloy", (.035,.045,.055),.40,.44)
silver = solid("Machined titanium edges", (.48,.50,.51),.55,.40)
brass = solid("Warm brass collars", (.40,.27,.13),.65,.40)
cyan = solid("Cyan luminous apertures", (.003,.15,.30),.1,.4,3.0)
glass = solid("Canopy edge seal", (.035,.055,.07),.25,.24)

def top_uv(v):
    return ((CX+v[0]*S)/1536,(CY-v[1]*S)/1024)

def side_uv(v):
    py=CY-v[1]*S
    px=interpolate(py,[(77,695),(250,938),(460,1134),(833,1468),(890,1514)])
    return (px/1536,(840-v[2]*S)/1024)

def front_uv(v):
    return ((1097+v[0]*71)/1536,(435-v[2]*60)/1024)

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
    if len(indices)==3:
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
                chosen,projection=mode(normal)
            elif mode=="auto":
                if normal[2]<-.35:
                    chosen=ventral
                elif abs(normal[0])>.68 and abs(normal[2])<.28:
                    projection=side_uv
                elif normal[1]>.75 and normal[2]<.45:
                    projection=front_uv
            elif mode=="fin":
                if normal[2]<-.45:
                    chosen=ventral
                elif abs(normal[2])<.4:
                    chosen=dark
            elif mode=="side":
                projection=side_uv
            elif mode=="top_bottom" and normal[2]<-.45:
                chosen=ventral
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
    binary=bytearray()
    doc={"asset":{"version":"2.0","generator":"Stellar Syndicates reference-to-mesh pipeline (Python/NumPy)"},"scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"name":"INTERCEPTOR","children":[],"extras":{"source":"interceptor-reference-v1/interceptor-reference.png","textureMethod":"UV projection of original reference artwork","note":"Unseen underside and precise depth inferred from the reference"}}],"meshes":[],"materials":materials,"buffers":[{}],"bufferViews":[],"accessors":[],"images":[],"samplers":[{"magFilter":9729,"minFilter":9987,"wrapS":33071,"wrapT":33071}],"textures":[]}
    def data_view(raw,target=None):
        while len(binary)%4: binary.append(0)
        start=len(binary)
        binary.extend(raw)
        view={"buffer":0,"byteOffset":start,"byteLength":len(raw)}
        if target: view["target"]=target
        doc["bufferViews"].append(view)
        return len(doc["bufferViews"])-1
    def accessor(data,component,kind,target,minmax=False):
        array=np.asarray(data,dtype="<f4" if component==5126 else "<u4")
        acc={"bufferView":data_view(array.tobytes(),target),"componentType":component,"count":len(array),"type":kind}
        if minmax:
            acc["min"]=array.min(axis=0).tolist()
            acc["max"]=array.max(axis=0).tolist()
        doc["accessors"].append(acc)
        return len(doc["accessors"])-1
    for filename in ["interceptor-basecolor.png","interceptor-metallic-roughness.png","interceptor-emissive.png"]:
        doc["images"].append({"name":filename,"mimeType":"image/png","bufferView":data_view((ROOT/"textures"/filename).read_bytes())})
        doc["textures"].append({"sampler":0,"source":len(doc["images"])-1})
    points=[]
    triangle_count=0
    for part in parts:
        primitives=[]
        for material,group in part["groups"].items():
            positions=np.array(group["position"])
            normals=np.array(group["normal"])
            # glTF uses Y-up: our +Y nose becomes glTF -Z.
            positions=positions[:,[0,2,1]]*np.array([1,1,-1])
            normals=normals[:,[0,2,1]]*np.array([1,1,-1])
            points.extend(positions)
            triangle_count+=len(group["indices"])//3
            primitives.append({"attributes":{"POSITION":accessor(positions,5126,"VEC3",34962,True),"NORMAL":accessor(normals,5126,"VEC3",34962),"TEXCOORD_0":accessor(group["uv"],5126,"VEC2",34962)},"indices":accessor(group["indices"],5125,"SCALAR",34963),"material":material})
        doc["meshes"].append({"name":part["name"],"primitives":primitives})
        doc["nodes"].append({"name":part["name"],"mesh":len(doc["meshes"])-1})
        doc["nodes"][0]["children"].append(len(doc["nodes"])-1)
    for name,loc in [("socket.engine.port",point(257,883,.79)),("socket.engine.starboard",point(445,883,.79)),("socket.weapon.port",point(246,290,1.13)),("socket.weapon.starboard",point(456,290,1.13))]:
        doc["nodes"].append({"name":name,"translation":[loc[0],loc[2],-loc[1]]})
        doc["nodes"][0]["children"].append(len(doc["nodes"])-1)
    doc["buffers"][0]["byteLength"]=len(binary)
    json_bytes=json.dumps(doc,separators=(",",":")).encode()
    while len(json_bytes)%4:json_bytes+=b" "
    while len(binary)%4:binary.append(0)
    length=12+8+len(json_bytes)+8+len(binary)
    glb=struct.pack("<4sII",b"glTF",2,length)+struct.pack("<I4s",len(json_bytes),b"JSON")+json_bytes+struct.pack("<I4s",len(binary),b"BIN\0")+binary
    (ROOT/"interceptor.glb").write_bytes(glb)
    points=np.array(points)
    stats={"meshes":len(parts),"triangles":triangle_count,"exportedVertices":len(points),"dimensionsXYZ":np.round(np.ptp(points,axis=0),4).tolist(),"textureResolution":[1536,1024],"embeddedTextures":3,"axes":"Y up, -Z forward","source":"../../interceptor-reference-v1/interceptor-reference.png","generator":"Python/NumPy, no Blender"}
    (ROOT/"model-info.json").write_text(json.dumps(stats,indent=2)+"\n")
    print(json.dumps(stats,indent=2))

    # OBJ/MTL is an additional broadly compatible editable interchange export.
    lines=["# Interceptor, Y-up, textures from the approved reference","mtllib interceptor.mtl"]
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
    (ROOT/"interceptor.obj").write_text("\n".join(lines)+"\n")
    mtl=[]
    for i,mat in enumerate(materials):
        pbr=mat["pbrMetallicRoughness"]
        mtl.extend([f"newmtl material_{i}","Kd "+" ".join(str(c) for c in pbr["baseColorFactor"][:3]),"Ks .25 .25 .25","Ns 48"])
        if "baseColorTexture" in pbr:mtl.extend(["map_Kd textures/interceptor-basecolor.png","map_Ke textures/interceptor-emissive.png"])
        mtl.append("")
    (ROOT/"interceptor.mtl").write_text("\n".join(mtl))
