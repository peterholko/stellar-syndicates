"""An exportable dark material variant; source geometry and PNG maps stay intact.

Linear vertex-color multipliers supply the armor, bevel and trim palette. This
uses standard glTF COLOR_0, without a viewer-only shader or added draw calls.
"""
from pathlib import Path
from collections import Counter
import hashlib
import json
import struct
import numpy as np
from render_core import Model

ROOT=Path(__file__).resolve().parent
SOURCE=ROOT/'authority-arbiter.glb'
OUTPUT=ROOT/'authority-arbiter-dark.glb'
PALETTE={
    'armor':[.12,.16,.225],
    'raisedArmor':[.145,.19,.26],
    'edgeMetal':[.33,.41,.52],
    'gold':[.83,.86,.91],
    'structure':[1.,1.,1.],
    'lights':[1.,1.,1.],
}


def classify(name,uv,normal):
    # Classify texture islands first: the gold roof of a spine and its ceramic
    # flanks share a component but must retain different material treatments.
    x,y=uv*2048
    if x>1730 and y>1730:return 'lights'
    if y<1024 and x>=1024:return 'gold'
    if y>=1024:return 'structure'
    if any(token in name for token in ('gold','gilt','cornice','crest','muzzle-rim')):return 'gold'
    # Nickel-colored structural accents break the dark outer silhouette at the
    # tower, turret crowns, prow, radiator bank and along the three spine roots.
    if any(token in name for token in ('buttress','roof-plate','closed-door','door-stiffener','rail.port','rail.starboard')):
        return 'edgeMetal'
    if name.startswith('mass-driver.upper-clamp'):return 'edgeMetal'
    if name=='mass-driver.barrel' and normal[2]<-.82:return 'edgeMetal'
    if name=='halo.continuous-armored-ring':return 'gold'
    if name.startswith('spine.') and '.armor-segment.' in name:return 'raisedArmor'
    # Non-axial faces are real chamfers, not a screen-space outline.
    n=np.abs(normal)
    if n.max()<.94:return 'edgeMetal'
    return 'armor'


def build():
    source=Model(SOURCE)
    doc=json.loads(json.dumps(source.doc))
    binary=bytearray(source.bin[:doc['buffers'][0]['byteLength']])
    counts=Counter()
    for mesh in doc['meshes']:
        for primitive in mesh['primitives']:
            attrs=primitive['attributes']
            uv=source.accessor(attrs['TEXCOORD_0'])
            normals=source.accessor(attrs['NORMAL'])
            indices=source.accessor(primitive['indices']).reshape(-1,3)
            colors=np.zeros((len(uv),3),float)
            hits=np.zeros(len(uv),float)
            for part in mesh['extras']['sourceComponents']:
                start=part['indexStart']//3
                end=start+part['indexCount']//3
                for triangle in indices[start:end]:
                    normal=normals[triangle].mean(axis=0)
                    normal/=np.linalg.norm(normal)
                    category=classify(part['name'],uv[triangle].mean(axis=0),normal)
                    color=np.array(PALETTE[category])
                    # Slight panel-to-panel tonal variation retains machinery
                    # depth at modest zoom without relying on added geometry.
                    if category in ('armor','raisedArmor'):
                        seed=hashlib.sha256(part['name'].encode()).digest()[0]/255
                        color*=.93+.14*seed
                    np.add.at(colors,triangle,color)
                    np.add.at(hits,triangle,1)
                    counts[category]+=1
            assert (hits>0).all()
            colors=(colors/hits[:,None]).astype('<f4')
            assert np.isfinite(colors).all() and colors.min()>=0 and colors.max()<=1
            binary.extend(b'\0'*(-len(binary)%4))
            view=len(doc['bufferViews'])
            doc['bufferViews'].append(dict(buffer=0,byteOffset=len(binary),byteLength=colors.nbytes,target=34962))
            binary.extend(colors.tobytes())
            attrs['COLOR_0']=len(doc['accessors'])
            doc['accessors'].append(dict(bufferView=view,componentType=5126,count=len(colors),type='VEC3'))
    doc['materials'][0]['name']='Authority blue-charcoal, nickel and antique gold'
    doc['materials'][1]['name']='Ventral dark hangar and keel plating'
    for mat in doc['materials']:
        mat['pbrMetallicRoughness']['roughnessFactor']=.94
    doc['nodes'][0]['name']='Authority Arbiter — dark imperial armor'
    doc['nodes'][0]['extras']['materialVariant']='dark-imperial'
    doc['nodes'][0]['extras']['paletteMethod']='Standard linear COLOR_0 tint over unchanged embedded texture maps'
    doc['buffers'][0]['byteLength']=len(binary)
    encoded=json.dumps(doc,separators=(',',':'),ensure_ascii=True).encode()
    encoded+=b' '*(-len(encoded)%4)
    binary.extend(b'\0'*(-len(binary)%4))
    data=struct.pack('<4sII',b'glTF',2,28+len(encoded)+len(binary))
    data+=struct.pack('<I4s',len(encoded),b'JSON')+encoded+struct.pack('<I4s',len(binary),b'BIN\0')+binary
    OUTPUT.write_bytes(data)
    info=json.loads((ROOT/'model-info.json').read_text())
    info.update(name='Authority Arbiter — dark imperial armor',variant='dark-imperial',
        palette=PALETTE,paletteTriangleCounts=dict(counts),
        sourceModelSHA256=hashlib.sha256(SOURCE.read_bytes()).hexdigest(),
        artDirection='Blue-charcoal armor, lighter nickel chamfers and armor edges, aged gold crests, preserved amber bridge and cyan running lights. Midtone armor remains readable against the near-black galaxy map.',
        textures='Original three embedded maps remain byte-identical; standard glTF COLOR_0 applies the dark palette on the model.',
        note='Material experiment on the approved mass-driver model. Geometry, UVs, hierarchy, sockets and triangle count are preserved. No game integration changes.')
    (ROOT/'dark-model-info.json').write_text(json.dumps(info,indent=2)+'\n')
    print(json.dumps(dict(output=OUTPUT.name,bytes=len(data),triangles=info['triangles'],renderSections=info['renderSections'],paletteTriangles=dict(counts))))


if __name__=='__main__':build()
