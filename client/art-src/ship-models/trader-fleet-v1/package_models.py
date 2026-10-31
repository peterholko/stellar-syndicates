"""Write the trader asset handoff notes, manifest, and portable GLB bundle."""
from pathlib import Path
import hashlib
import json
import zipfile

ROOT=Path(__file__).resolve().parent
SHIPS=['clean-slate','second-wind','small-fortune']
DESCRIPTIONS={
    'clean-slate':'Ivory and teal precision freighter: six raised sealed cargo lids, two side docking collars, a low glazed bridge, integrated rectangular drives, and a ventral freight elevator.',
    'second-wind':'Moss-green salvage workboat: an offset port bridge, forward equipment pallet and cradle rails, a folded starboard crane with two gripping jaws, a port docking collar, workshop hatch, and differently housed engines.',
    'small-fortune':'Rust-red trading runner: a blunt spade-shaped cargo body, four cargo lids, a wraparound bridge, a port docking collar, two long cylindrical engines on swept pylons, and open gaps around the narrow service tail.',
}
manifest={'collection':'Independent trader ships','created':'2026-10-27','textureRevision':2,
    'format':'glTF 2.0 binary, self-contained embedded textures','axes':'Y up, -Z forward',
    'units':'Relative model units; assign gameplay scale during integration',
    'stage':'Textured static concept models','assets':[]}
for slug in SHIPS:
    folder=ROOT.parent/(slug+'-textured-v1')
    info=json.loads((folder/'model-info.json').read_text())
    validation=json.loads((folder/'gltf-validation.json').read_text())
    surfaces=json.loads((folder/'surface-verification.json').read_text())
    assert validation['issues']['numErrors']==0 and validation['issues']['numWarnings']==0
    assert surfaces['triangles']==info['triangles']
    source=ROOT.parent.parent/f'{slug}-reference-v1/{slug}-reference.png'
    model=folder/(slug+'.glb')
    cabin_note=('\nSecond Wind’s command module was subsequently fitted to the actual roof and window boundaries. [Close-up comparison and exported-window checks](../trader-fleet-v1/command-module-repair/report.html) records that correction.\n' if slug=='second-wind' else '')
    manifest['assets'].append(dict(id=slug,name=info['name'],owner=info['owner'],
        file=f'{slug}/{slug}.glb',projectFile=str(model),triangles=info['triangles'],
        renderSections=2,textureResolution=[2048,2048],dimensionsXYZ=info['dimensionsXYZ'],
        bytes=model.stat().st_size,sha256=hashlib.sha256(model.read_bytes()).hexdigest(),
        reference=str(source),referenceSHA256=hashlib.sha256(source.read_bytes()).hexdigest(),
        gltfErrors=0,gltfWarnings=0,socketCount=len(info['sockets'])))
    (folder/'build_model.py').write_text(f'''"""Rebuild {info['name']} with the shared trader reconstruction source."""
from pathlib import Path
import sys
sys.path.insert(0,str(Path(__file__).resolve().parent.parent/'trader-fleet-v1'))
from build_models import build_one
if __name__=='__main__':
    build_one('{slug}')
''')
    md=f'''# {info['name']} — {info['owner']}

{DESCRIPTIONS[slug]}

[Open the interactive model](viewer.html) · [GLB]({slug}.glb) · [Rendered views](previews/contact-sheet.png) · [Approved reference](textures/reference-sheet.png)

## Asset

- **{info['triangles']:,} triangles**, {info['sourceComponents']} closed source components, two merged material sections.
- Self-contained GLB with three embedded 2048 × 2048 maps: base color, metallic/roughness, and restrained cyan emission.
- OBJ/MTL interchange copy and external texture PNGs. OBJ preserves named mesh parts; attachment nodes and complete PBR materials are in GLB.
- Y up, −Z forward. Dimensions: {' × '.join(str(v) for v in info['dimensionsXYZ'])} relative units.
- {len(info['sockets'])} attachment nodes. Directions live in each node's `extras.direction`, in model axes.
- Static model. The crane, doors and engines have no animation or gameplay effects.

## Reconstruction

The approved TOP view defines the footprint and component stations. The other views guide volume and underside plating; proportions are reconciled where the concept views disagree. Major features are explicit geometry; smaller panel lines and service detail come from native reference pixels. The source sheet is retained unchanged. Supporting paint/service trims were generated with the built-in image_gen tool; the saved sheet and exact prompt are in the shared fleet folder. Hidden metal fittings reuse the existing fleet material strip.

Texture revision 2 repairs undersized material crops, collapsed UV substitutions, thin panel edges, cap/ring scaling, canopy geometry, and side/underside registration. Hull faces are split at piecewise registration landmarks so interpolation cannot displace features inside long walls. [Detailed texture audit and before/after views](../trader-fleet-v1/texture-audit-v1/report.html) documents both fixes and remaining concept-projection seams. The original models are preserved in that audit folder.
{cabin_note}

The model follows the project's Python/NumPy reconstruction pipeline. The shared implementation and inspection tools are in [trader-fleet-v1](../trader-fleet-v1/README.md). No Blender installation is required. Runtime scale, LOD selection, sprite baking and catalog integration are subsequent production steps.

## Verification

`gltf-validation.json` records zero errors and zero warnings. `surface-verification.json` independently checks the exported buffers for finite data, valid winding, closed positive-volume components, nondegenerate triangles, noncollapsed UVs, underside coverage, required features and attachment directions. `three-validation.json` checks loading and socket transforms with the game's Three.js r180. Software renders in `previews/` use the GLB's actual meshes and embedded textures; `webgl-preview.png` records the live browser view. UV distortion/density and shared-edge color measurements are recorded separately in the texture audit; file validity alone does not establish visual alignment.

Rebuild from the repository root:

```sh
python3 client/art-src/ship-models/{slug}-textured-v1/build_model.py
python3 client/art-src/ship-models/trader-fleet-v1/inspect_models.py {slug}
node client/art-src/ship-models/mining-fleet-v1/validate.mjs ../{slug}-textured-v1/{slug}.glb
node client/art-src/ship-models/trader-fleet-v1/test_assets.mjs
node client/art-src/ship-models/trader-fleet-v1/build_previews.mjs
```
'''
    (folder/'README.md').write_text(md)
(ROOT/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
bundle_notes='''INDEPENDENT TRADERS — STELLAR SYNDICATES

Texture revision 2. Three self-contained textured GLB models: Clean Slate (Ines Varga),
Second Wind (Mags Halloran), and Small Fortune (Rafi Mensah).

Open each .glb in a glTF-compatible viewer or 3D editor. All PBR textures are
embedded. Axes: Y up, -Z forward. Scale is in relative model units. Each
model-info.json records component counts, dimensions and attachment sockets.

These are static concept reconstructions from the approved reference sheets.
Game catalog integration, LODs and animation are separate production steps.
The texture audit repairs major mapping faults; some source-view projection
seams remain at close range. See the included audit summary for details.
The workspace source folders also contain editable build scripts, OBJ/MTL
exports, external texture maps, validation reports and interactive previews.
'''
with zipfile.ZipFile(ROOT/'trader-ships-glb.zip','w',compression=zipfile.ZIP_DEFLATED,compresslevel=6) as bundle:
    bundle.writestr('README.txt',bundle_notes)
    bundle.writestr('manifest.json',json.dumps(manifest,indent=2)+'\n')
    bundle.write(ROOT/'texture-audit-v1/summary.json','texture-audit-summary.json')
    bundle.write(ROOT/'command-module-repair/after/inspection.json','second-wind/command-module-inspection.json')
    for slug in SHIPS:
        folder=ROOT.parent/(slug+'-textured-v1')
        for name in (slug+'.glb','model-info.json'):
            bundle.write(folder/name,f'{slug}/{name}')
with zipfile.ZipFile(ROOT/'trader-ships-glb.zip') as bundle:
    assert bundle.testzip() is None
    for item in manifest['assets']:
        assert hashlib.sha256(bundle.read(item['file'])).hexdigest()==item['sha256']
print(json.dumps({'models':len(manifest['assets']),'bundle':str(ROOT/'trader-ships-glb.zip'),
    'bytes':(ROOT/'trader-ships-glb.zip').stat().st_size,'zipCRCAndModelHashesVerified':True}))
