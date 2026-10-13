"""Package portable sources, unchanged references and validated pirate models."""
from pathlib import Path
import hashlib
import json
import shutil
import zipfile

ROOT = Path(__file__).resolve().parent
SHIPS = ['privateer-raider', 'pirate-corsair', 'pirate-boarding-raider']


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    rows = []
    manifest = {'generator': 'Python/NumPy reference-to-mesh pipeline; no Blender',
                'referenceMethod': 'Built-in image generation tool, conditioned on the original pirate sprites',
                'ships': []}
    for ship in SHIPS:
        folder = ROOT.parent/f'{ship}-textured-v1'
        spec = json.loads((folder/'model-spec.json').read_text())
        info = json.loads((folder/'model-info.json').read_text())
        checks = json.loads((folder/'geometry-validation.json').read_text())
        gltf = json.loads((folder/'gltf-validation.json').read_text())
        assert gltf['issues']['numErrors'] == gltf['issues']['numWarnings'] == 0
        assert checks['triangles'] == info['triangles']
        reference = ROOT.parent.parent/f'{ship}-reference-v1'/f'{ship}-reference.png'
        assert reference.read_bytes() == (folder/'textures/reference-sheet.png').read_bytes()
        rows.append(f"| {spec['title']} | {info['triangles']:,} | {info['renderSections']} | [{ship}.glb](../{ship}-textured-v1/{ship}.glb) |")
        manifest['ships'].append({
            'id': ship, 'title': spec['title'], 'triangles': info['triangles'],
            'renderSections': info['renderSections'], 'sourceComponents': info['sourceComponents'],
            'model': f'../{ship}-textured-v1/{ship}.glb',
            'glbSha256': sha((folder/f'{ship}.glb').read_bytes()),
            'referenceSha256': sha(reference.read_bytes()),
            'validation': {'gltfErrors': 0, 'gltfWarnings': 0,
                           'closedComponents': checks['closedComponents'],
                           'dedicatedBottomTriangles': checks['dedicatedBottomTriangles']}})
        readme = f'''# {spec['title']}

Textured low-polygon reconstruction of the included five-view pirate reference.

- **{info['triangles']:,} triangles**, {info['renderSections']} consolidated render sections.
- GLB embeds 2048 × 2048 base-color, metallic/roughness and emissive PNG textures.
- Separate top and underside UV projections preserve the reference's ventral details.
- Recessed engine mouths, solid hulls and weapons are real geometry. The Boarding Raider's cages have open frames.
- Y up, forward along −Z; approximately 20 authoring units long. Physical game sizes are not assigned.
- Named component geometry is retained in the OBJ and deterministic Python source; GLB static parts are combined by material.
- Engines and muzzle/capture sockets are exported where applicable. This is a static model with no animation rig.

## Files

`{ship}.glb` is the self-contained runtime asset. `{ship}.obj` / `.mtl` and `textures/` are editable interchange files. `previews/` contains eight software-rendered views read from the actual GLB. `textures/reference-sheet.png` is the unchanged generated reference; `source/` retains the original game sprite and exact image prompts.

## Rebuild

```sh
python3 -m pip install -r requirements.txt
python3 prepare_textures.py
python3 build_model.py
```

No Blender or external modeling application is required. `designs.py` traces the hull in reference pixel coordinates. `model-spec.json` registers each reference view; `mesh_core.py` triangulates, projects UVs and exports; `geometry.py` supplies closed geometry helpers.

## Validation

Khronos glTF Validator: **0 errors, 0 warnings**. The independent geometry check confirms finite attributes, valid indices, unit normals, consistent winding, no degenerate triangles, noncollapsed bottom UVs and positive-volume closed surfaces for all {checks['closedComponents']} authored components. Separate solid components can intersect; this is not a single watertight 3D-printing surface.

The supplied previews use an independent software rasterizer. They are not a WebGL/PBR acceptance test. See the fleet folder for the interactive offline viewer.

Reference images were generated with the built-in image generation tool. Material-channel textures are derived from those images; small structural metal parts use matching solid materials. Some perspective and lighting information is baked into the reference artwork.
'''
        (folder/'README.md').write_text(readme)
    intro = '''# Pirate fleet — Stellar Syndicates

Three five-view reference sheets and three textured low-polygon 3D models, created without Blender. The references preserve the original pirate silhouettes, worn metal, patchwork armor and amber engines.

Open **viewer.html** in a browser to orbit each model, inspect the five orthographic directions, show its wireframe, compare its reference sheet and download the GLB. All models, PNGs and the Three.js runtime are embedded, so it needs no server or network connection.

'''
    table = '| Model | Triangles | Render sections | GLB |\n|---|---:|---:|---|\n'+'\n'.join(rows)+'\n'
    ending = '''
Each model package includes GLB, OBJ/MTL, material textures, the unchanged reference, editable Python build files, eight rendered views and validation reports. `pirate-fleet-models.png` compares their upper surfaces, engine views and undersides.

The five reference directions are **front, back, top, bottom and side**. The image prompts and subsequent view-correction prompts are preserved verbatim in `prompts.json`. References were generated with the built-in image generation tool and conditioned on the original game sprites. Models were authored from those references with Python/NumPy. No Blender was used.

## Validation

All three final GLBs pass Khronos glTF Validator with **0 errors and 0 warnings**. Independent buffer-level checks verify topology, indices, normals and UVs, including the dedicated underside projections. All 161 authored components are individually closed solids with positive volume. Eight views of each exported model were rendered in software; the contact sheet shows three of them.

The interactive viewer was bundled and its embedded files were checked. Browser interaction could not be tested because browser access remains disabled by the saved permission. The software previews are separate model renders, not screenshots of the viewer.

## Scope and scale

These are standalone pirate assets. They have not been connected to the galaxy-map or battle model catalog. Their authoring length is approximately 20 units each; physical game sizes still need assigning during integration. Static geometry is combined by material, and the OBJ retains named construction pieces. No animated rig is included.

## Rebuilding

To rebuild one ship, run `prepare_textures.py` then `build_model.py` from its folder after installing its `requirements.txt`. To rebuild this gallery, install the pinned tools with `npm ci --ignore-scripts`, then run `node build_viewer.mjs` from the fleet pipeline folder. The shared scaffold is optional and uses the bundled exporter templates. `python3 render_models.py` repeats the independent mesh checks and model renders; `node validate.mjs` repeats glTF validation.

Source-component counts, UV registration and hashes are in the JSON manifests. The reference images are unchanged; texture padding and derived material channels live separately in each model's `textures/` directory.
'''
    (ROOT/'README.md').write_text(intro+table+ending)
    (ROOT/'asset-manifest.json').write_text(json.dumps(manifest, indent=2)+'\n')
    # Include the exact template version so the optional scaffold is portable.
    template = ROOT/'templates'
    template.mkdir(exist_ok=True)
    for name in ['mesh_core.py', 'geometry.py']:
        if not (template/name).exists():
            shutil.copyfile(ROOT.parent/'scout-textured-v1'/name, template/name)
    three_license = ROOT/'node_modules/three/LICENSE'
    if three_license.exists():
        (ROOT/'THIRD_PARTY_LICENSES.txt').write_text('Three.js (offline viewer)\n\n'+three_license.read_text())
    files = {}
    def include(filename, archive_name):
        files[archive_name] = filename.read_bytes()
    include(ROOT/'viewer.html', 'pirate-fleet/viewer.html')
    include(ROOT/'pirate-fleet-models.png', 'pirate-fleet/pirate-fleet-models.png')
    files['pirate-fleet/README.md'] = (intro+table.replace('](../', '](ship-models/')+ending.replace('in `prompts.json`', 'in `ship-models/pirate-fleet-v1/prompts.json`')).encode()
    for ship in SHIPS:
        for folder in [ROOT.parent/f'{ship}-textured-v1', ROOT.parent.parent/f'{ship}-reference-v1']:
            prefix = 'ship-models/' if '-textured-v1' in folder.name else ''
            for filename in sorted(folder.rglob('*')):
                if filename.is_file() and '__pycache__' not in filename.parts and filename.name != '.DS_Store':
                    include(filename, f'pirate-fleet/{prefix}{folder.name}/{filename.relative_to(folder)}')
    for filename in sorted(ROOT.rglob('*')):
        relative = filename.relative_to(ROOT)
        if not filename.is_file() or any(p in {'node_modules', '__pycache__', '.git'} for p in relative.parts):
            continue
        if filename.name in {'viewer.html', 'pirate-fleet-models.png', 'pirate-fleet-assets.zip', 'package-manifest.json', '.DS_Store'}:
            continue
        include(filename, f'pirate-fleet/ship-models/pirate-fleet-v1/{relative}')
    hashes = {name: {'bytes': len(data), 'sha256': sha(data)} for name, data in files.items()}
    files['pirate-fleet/checksums.json'] = (json.dumps(hashes, indent=2)+'\n').encode()
    archive = ROOT/'pirate-fleet-assets.zip'
    with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED, compresslevel=6) as z:
        for name, data in files.items():
            z.writestr(name, data)
    with zipfile.ZipFile(archive) as z:
        assert z.testzip() is None
        for name, details in hashes.items():
            assert sha(z.read(name)) == details['sha256']
    summary = {'archive': archive.name, 'bytes': archive.stat().st_size,
               'sha256': sha(archive.read_bytes()), 'entries': len(files), 'integrity': 'passed'}
    (ROOT/'package-manifest.json').write_text(json.dumps(summary, indent=2)+'\n')
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
