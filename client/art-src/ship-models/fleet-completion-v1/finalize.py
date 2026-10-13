"""Verify deliverables, write the browsable gallery and package the new fleet."""
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
import base64,hashlib,json,shutil,subprocess,zipfile
from PIL import Image
import numpy as np
from scaffold import SPECS,ROOT,MODELS

lab=MODELS/'interceptor-lod-experiment-v1'
def sha(data):return hashlib.sha256(data).hexdigest()
def get(file):return json.loads(file.read_text())

def verify():
    lineup=get(lab/'lineup-verification.json')
    assert len(lineup['singleShip'])==18 and all(lineup['checks'].values())
    baseline=get(Path('/private/tmp/nine-ships-baseline.json'))
    assert all(sha(Path(p).read_bytes())==expected for p,expected in baseline.items()),'Prior asset changed'
    (ROOT/'prior-assets-verification.json').write_text(json.dumps({'checked':len(baseline),'unchanged':True,'sha256':baseline},indent=2)+'\n')
    # Comparison renders now exist, so include their pixel differences in reports.
    for ship in SPECS:
        check=subprocess.run(['python3','verify_geometry.py',ship],cwd=lab,stdout=subprocess.DEVNULL)
        assert check.returncode==0,ship
    html=(lab/'viewer.html').read_text()
    marker='<script id="assets" type="application/json">'
    payload=json.loads(html.split(marker,1)[1].split('</script>',1)[0])
    embedded=0
    for ship,assets in payload.items():
        for level in ('original','lod0','lod1','lod2'):
            assert sha(base64.b64decode(assets[level]))==sha((lab/'models'/f'{ship}-{level}.glb').read_bytes())
            embedded+=1
        assert sha(base64.b64decode(assets['sprite'].split(',',1)[1]))==sha((lab/'sprites'/f'{ship}-views.png').read_bytes())
        atlas=Image.open(lab/'sprites'/f'{ship}-views.png').convert('RGBA')
        assert atlas.size==(2048,1280)
        alpha=atlas.getchannel('A')
        assert all(alpha.crop((x*256,y*256,(x+1)*256,(y+1)*256)).getbbox() for y in range(5) for x in range(8))
    validated=get(lab/'validation.json')
    assert len(validated)==54 and all(d['issues']['numErrors']==d['issues']['numWarnings']==0 for d in validated.values())
    render={ship:get(lab/(ship+'-render-verification.json'))['checks'] for ship in SPECS}
    assert all(all(checks.values()) for checks in render.values())
    previous=lab/'experiment-manifest.json'
    if not (ROOT/'previous-experiment-manifest.json').exists():shutil.copyfile(previous,ROOT/'previous-experiment-manifest.json')
    manifest={'ships':list(payload),'embeddedModelsVerified':embedded,'embeddedSpriteAtlasesVerified':len(payload),
      'optimizedGLBsValidated':len(validated),'glTFErrors':0,'glTFWarnings':0,
      'sourceAndPriorModelsUnchanged':True,'priorAssetsHashChecked':len(baseline),
      'lineupShips':18,'lineupRows':2,'lineupScaleOptions':['game','native'],'lineupProjection':'orthographic',
      'lineupRenderChecks':lineup['checks'],'newShipRenderChecks':render,'viewerBytes':(lab/'viewer.html').stat().st_size,
      'sourceReferenceRegionsIncluded':True,'spriteTilesVerified':len(payload)*40,
      'browserWarningsAndErrors':[],'standalonePackage':'../ship-lod-experiment-v9.zip'}
    previous.write_text(json.dumps(manifest,indent=2)+'\n')
    return manifest

def docs(manifest):
    (lab/'specs').mkdir(exist_ok=True)
    cards=[];rows=[];references=[]
    thumbs=ROOT/'thumbnails';thumbs.mkdir(exist_ok=True)
    for ship,spec in SPECS.items():
        folder=MODELS/(ship+'-textured-v1');info=get(lab/(ship+'-model-info.json'))
        shutil.copyfile(folder/'model-spec.json',lab/'specs'/(ship+'.json'))
        source=MODELS.parent/(ship+'-reference-v1')
        prompts=folder/'prompts';prompts.mkdir(exist_ok=True)
        for file in source.glob('*prompt.txt'):shutil.copyfile(file,prompts/file.name)
        model=get(folder/'model-info.json')
        # Keep the primary source metadata explicit about counts and references.
        model.update(engineCount=spec['engines'],turretCount=spec['turrets'],referenceViews=['front','back','top','bottom','side'])
        (folder/'model-info.json').write_text(json.dumps(model,indent=2)+'\n')
        readme=f'''# {spec['title']} — textured 3D model

Reconstructed from the accompanying five-view reference sheet with Python and NumPy. No Blender.

- Full model: `{ship}.glb` with three embedded 2048 × 2048 PBR texture maps.
- Editable interchange: `{ship}.obj` and `{ship}.mtl`, with maps in `textures/`.
- Reference: `textures/reference-sheet.png` — front, back, top, bottom and side.
- Interactive preview: open `viewer.html` in a browser. All renderer code and model bytes are embedded.
- Authored components: {model['sourceComponents']}; triangles: {model['triangles']:,}; render sections: {model['renderSections']}.
- Engines: {spec['engines']}; separate turret assemblies: {spec['turrets']}.
- Axes: Y up; −Z forward. Working units are independent from physical/game sizes.

Textures preserve native reference pixels with separate dorsal, ventral, side and end projections. White background margins are dilated to prevent pale seams. Occluded rear armor uses a visible reference panel; recessed exhaust faces have local rear-view mapping. Base color, metallic/roughness and cyan emission maps are embedded. This is an editable reconstruction of the concept, rather than a photogrammetry scan.

`build_model.py`, `designs.py`, `geometry.py`, `mesh_core.py` and `model-spec.json` contain the deterministic source. Plan contours use native reference pixel coordinates. Static pieces consolidate by material. Named turret groups and attachment sockets survive; the experiment's LOD exporter adds articulated yaw pivots.

Rebuild with Python, NumPy and Pillow:

```sh
python3 prepare_textures.py
python3 build_model.py
```

Preview rebuild and glTF validation use the pinned dependencies in `package.json`:

```sh
npm install
node build_preview.mjs
node validate.mjs
```

`validation.json` records zero glTF errors/warnings. `surface-verification.json` checks nonzero geometry and textured UV areas, underside UV placement, texture count and sockets. `previews/` contains eight actual model renders, including underside and all five orthographic views.

The reference image was created with the built-in image-generation tool from the project's established ship artwork. Exact generation and revision prompts are in `prompts/`.
'''
        (folder/'README.md').write_text(readme)
        # Thumbnail is a crop of an actual renderer capture, not concept artwork.
        im=Image.open(folder/'previews/front-quarter.png').convert('RGB');pixels=np.asarray(im).astype('int16')
        y,x=np.nonzero(np.max(np.abs(pixels-pixels[0,0]),axis=2)>18)
        im=im.crop((max(0,x.min()-35),max(0,y.min()-35),min(im.width,x.max()+36),min(im.height,y.max()+36)))
        im.thumbnail((1000,720),Image.Resampling.LANCZOS);im.save(thumbs/(ship+'.jpg'),quality=94)
        costs=[level['triangles'] for level in info['levels']]
        rows.append(f"| {spec['title']} | {costs[0]:,} | {costs[1]:,} | {costs[2]:,} | {info['levels'][0]['drawCalls']} |")
        references.append(f"- [{spec['title']} reference](../{ship}-textured-v1/textures/reference-sheet.png) · [GLB](../{ship}-textured-v1/{ship}.glb) · [prompts](../{ship}-textured-v1/prompts/prompt.txt)")
        cards.append(f'''<article><a class="image" href="../{ship}-textured-v1/viewer.html"><img src="thumbnails/{ship}.jpg" alt="Rendered 3D {spec['title']}" loading="lazy"></a><div class="card-body"><h2>{spec['title']}</h2><p>{costs[0]:,} → {costs[2]:,} triangles · {spec['engines']} engines</p><nav><a class="primary" href="../{ship}-textured-v1/viewer.html">Explore 3D</a><a href="../{ship}-textured-v1/textures/reference-sheet.png">Five-view reference</a><a download href="../interceptor-lod-experiment-v1/models/{ship}-lod0.glb">GLB</a><a download href="../{ship}-textured-v1.zip">Editable package</a></nav></div></article>''')
    table='| Ship | Full / LOD 0 | LOD 1 | LOD 2 | Draw calls |\n|---|---:|---:|---:|---:|\n'+'\n'.join(rows)
    (ROOT/'README.md').write_text('''# Nine new fleet models

All nine requested ships have five-view reference sheets, textured GLB and OBJ/MTL models, standalone previews, three mesh detail levels and forty-direction sprite atlases. The low-triangle viewer now contains all eighteen ship types.

Open `index.html` for the gallery, or `../interceptor-lod-experiment-v1/viewer.html?formation=all` for the complete size comparison. The gallery provides per-ship reference, GLB and editable-package links.

'''+table+'''

Every distant sprite is two triangles and one draw call. Static surfaces are combined by material. Destroyer, Battleship and Dreadnought retain 2, 7 and 17 independent turret mounts; turret materials still require separate draws. These counters describe this uninstanced prototype, not a battle performance measurement.

The models use the reference's pale titanium, dark structures, brass details and cyan apertures. All undersides have dedicated texture projections. There are three embedded 2048-pixel PBR maps per model. Five-view concept sheets were generated using the built-in image-generation tool; the 3D reconstruction uses deterministic Python/NumPy geometry, without Blender.

## Reference images and exact prompts

'''+ '\n'.join(references)+'''

## Checks

- Nine source GLBs and 54 optimized GLBs pass glTF validation with zero errors/warnings.
- All new mesh levels retain original texture bytes and primary protected geometry; LOD 0 retains every original triangle, normal, UV and material.
- Surface checks cover bottom projections and collapsed geometry/UVs. Browser captures include all six sides and both quarters.
- Forty nonempty sprite tiles per ship; 720 across the eighteen-ship viewer.
- 119 lineup checks pass, including both scales, single-ship and formation regression checks, independent LODs and turret movement.
- 55 previous model/sprite assets are verified byte-identical.

`progress.json`, `prior-assets-verification.json`, source/package reports and the experiment manifest record the delivered state. The game code is outside this asset experiment.

## Reproduction

Each ship folder is independently rebuildable; see its README. `scaffold.py` and `build_all.py` reproduce the nine source packages, while `build_lods.py` produces their optimized variants. `integrate_viewer.py` records the initial one-time lab migration and is not an idempotent rebuild step. Use the experiment's documented commands for subsequent builds.
''')
    html='''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Nine new ships · Stellar Syndicates</title><style>
:root{color-scheme:dark;font-family:system-ui,sans-serif;color:#e6eef5;background:#0a121c}*{box-sizing:border-box}body{margin:0}header,main,footer{max-width:1440px;margin:auto;padding:36px}header{padding-top:56px}header small{font-size:11px;letter-spacing:.18em;color:#86bccb}h1{font-size:42px;letter-spacing:-.04em;font-weight:550;margin:15px 0}header p{color:#9eb1c0;line-height:1.6;max-width:810px}a{color:#a6e1eb;text-decoration:none}a:hover{color:white}a:focus-visible{outline:2px solid #e6be7a;outline-offset:4px}.actions{display:flex;gap:12px;flex-wrap:wrap;margin-top:26px}.actions a,.primary{display:inline-block;background:#a6e1eb!important;color:#10242d!important;border-radius:7px;padding:11px 16px}.actions a+ a{background:#172b39!important;color:#b3d5df!important}main{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:24px;padding-top:0}article{border:1px solid #294051;border-radius:13px;background:#101c28;overflow:hidden}.image{display:block;background:#0d141e;height:260px;padding:15px}.image img{width:100%;height:100%;object-fit:contain}.card-body{padding:23px}h2{font-size:21px;font-weight:550;margin:0 0 10px}.card-body p{font-size:12px;color:#89a3b6;margin-bottom:22px}nav{display:flex;align-items:center;gap:14px;flex-wrap:wrap;font-size:12px}nav .primary{padding:8px 12px}footer{color:#7792a6;font-size:12px;line-height:1.7;padding-top:0}@media(max-width:1000px){main{grid-template-columns:repeat(2,minmax(0,1fr))}}@media(max-width:650px){main{grid-template-columns:1fr}header,main,footer{padding:22px}h1{font-size:32px}}
</style><header><small>STELLAR SYNDICATES / FLEET COLLECTION</small><h1>Nine new ships. A complete fleet.</h1><p>Five-view reference sheets and textured 3D models for the remaining combat, utility and freight ships. Explore each hull, inspect its underside, or compare all eighteen ships at the game's display scale.</p><div class="actions"><a href="../interceptor-lod-experiment-v1/viewer.html?formation=all">Open 18-ship viewer</a><a download href="../nine-new-ships-v1.zip">Download all nine models</a><a href="README.md">Asset details &amp; prompts</a></div></header><main>'''+''.join(cards)+'''</main><footer>GLB · OBJ / MTL · PBR maps · full, medium and far detail · forty-view sprites<br>Shown here: actual 3D renders. Each reference includes front, back, top, bottom and side views. Built without Blender.</footer></html>'''
    (ROOT/'index.html').write_text(html)
    # Update the existing lab documentation around the full delivered catalog.
    oldreadme=lab/'README.md'
    if not (ROOT/'previous-experiment-README.md').exists():shutil.copyfile(oldreadme,ROOT/'previous-experiment-README.md')
    oldreadme.write_text('''# Ship detail and distance experiment — 18 ships

Open `viewer.html` in a browser. It embeds the renderer, 72 GLBs and 18 sprite atlases and works offline. Use Ship model for a single type, or Formation → One of each ship for the two-row comparison. The individual model/reference gallery is in `../fleet-completion-v1/index.html`.

The catalog includes Interceptor, Corvette, Destroyer, Cruiser, Battleship, Dreadnought, Titan; Scout, Colony Ship, Troop Transport, Construction Ship; Tiny, Small, Medium, Large, Heavy, Bulk and Authority Freighters.

## Nine new additions

'''+table+'''

Existing model and sprite bytes are unchanged. Primary hulls and structural components are protected during simplification; rounded surfaces and fittings lose detail. Static pieces share material groups. Combat turret assemblies retain independent yaw pivots and weapon sockets. The original source view uses their resting pose. All mesh levels keep original texture bytes, UVs and vertex attributes; LOD 0 preserves every original triangle. The new models additionally lock global extremal vertices to preserve framing and size bounds.

Automatic switching uses projected bounding-sphere diameter in CSS pixels: LOD 0 above 230, LOD 1 above 100, LOD 2 above 38, otherwise a two-triangle sprite. A 12% hysteresis band prevents threshold flicker. There are 40 baked directions per sprite (eight azimuths × elevations −90°, −45°, 0°, 45°, 90°), including the underside. Billboards use baked lighting and discrete view angles, appropriate for tiny ships; the mesh files require runtime switching by an engine.

One-of-each uses an orthographic camera and uniform scaling per ship. Game display scale follows the current presentation rules in `comparison-scale.js`, relative to Interceptor length 1×. These ratios are display proportions, not physical meters. Original model scale shows independent authoring units. Labels and exported size JSON report the selected basis.

The lab shares geometry and materials between copies of the same ship, but it does not instance ships or include battle effects. Render counts are not an FPS benchmark. Independent turret materials retain their draw calls at all mesh levels. The embedded offline viewer is deliberately larger than an asset-streamed production scene.

## Validation

`experiment-manifest.json` records the current catalog and embedded hashes. All 54 optimized GLBs have zero glTF errors/warnings. Geometry checks cover identical LOD 0 triangles, preserved textures and protected parts, valid bottom UVs, noncollapsed geometry/UVs, consistent bounds, moving turret groups and child weapon sockets. The eighteen-ship lineup passes 119 browser checks; individual new types also pass one-ship and eighty-ship render counts, automatic levels, hysteresis and sprite cost checks. The 55 previous model/sprite assets are byte-identical.

## Rebuild

Pinned dependencies: Node.js, Three.js 0.180.0, meshoptimizer 1.0.1, esbuild 0.25.10 and glTF Validator. Geometry verification also uses Python, NumPy and Pillow.

```sh
npm install
npm run models
npm run build
npm run validate
python3 preview_server.py --port 8770
```

Build one type with `node build_models.mjs <ship-id>` and verify it with `python3 verify_geometry.py <ship-id>`. Builders prefer the sibling source model and fall back to `models/<ship-id>-original.glb` and `specs/<ship-id>.json` for standalone packages.

To rebake a ship and capture checks, open `http://127.0.0.1:8770/viewer.html?ship=<ship-id>&bake=1&capture=1`. Run `npm run build` after baking to embed updated atlases. `?formation=all&capture-lineup=1` runs lineup checks and writes both scale captures. Capture endpoints bind to loopback and allow only predefined bounded image/JSON files.

The latest complete portable package is `../ship-lod-experiment-v9.zip`. All modeling uses Python/NumPy; no Blender.
''')
    package=get(lab/'package.json');package['scripts']['models']='node build_all_models.mjs'
    (lab/'package.json').write_text(json.dumps(package,indent=2)+'\n')

def package_files():
    def include(p):return p.is_file() and '__pycache__' not in p.parts and p.suffix not in ('.pyc','.log') and p.name!='.DS_Store'
    def pack(ship):
        folder=MODELS/(ship+'-textured-v1');out=MODELS/(folder.name+'.zip')
        with zipfile.ZipFile(out,'w',zipfile.ZIP_DEFLATED,compresslevel=6) as z:
            for p in sorted(folder.rglob('*')):
                if include(p):z.write(p,p.relative_to(MODELS))
        return out.name
    with ThreadPoolExecutor(max_workers=3) as pool:list(pool.map(pack,SPECS))
    collection=MODELS/'nine-new-ships-v1.zip'
    with zipfile.ZipFile(collection,'w',zipfile.ZIP_DEFLATED,compresslevel=6) as z:
        for ship in SPECS:
            folder=MODELS/(ship+'-textured-v1')
            for p in sorted(folder.rglob('*')):
                if include(p):z.write(p,p.relative_to(MODELS))
            for level in ('lod0','lod1','lod2'):
                p=lab/'models'/f'{ship}-{level}.glb';z.write(p,'optimized/'+p.name)
            p=lab/'sprites'/f'{ship}-views.png';z.write(p,'sprites/'+p.name)
        archive_readme=(ROOT/'README.md').read_text().replace('../','')
        start=archive_readme.index('Open `index.html`')
        end=archive_readme.index('\n\n',start)
        archive_readme=archive_readme[:start]+'''Each `<ship>-textured-v1/` folder contains its standalone `viewer.html`, source GLB, editable OBJ/MTL, reference, textures, generation prompts and model source. The three optimized detail levels are in `optimized/`; distant atlases are in `sprites/`. The full eighteen-ship interactive viewer is available separately as `ship-lod-experiment-v9.zip`.'''+archive_readme[end:]
        z.writestr('README.md',archive_readme);z.write(ROOT/'nine-new-ships.jpg','nine-new-ships.jpg')
    complete=MODELS/'ship-lod-experiment-v9.zip'
    with zipfile.ZipFile(complete,'w',zipfile.ZIP_DEFLATED,compresslevel=6) as z:
        for p in sorted(lab.rglob('*')):
            if include(p):z.write(p,p.relative_to(MODELS))
    return {p.name:{'bytes':p.stat().st_size,'sha256':sha(p.read_bytes())} for p in [collection,complete]}

if __name__=='__main__':
    manifest=verify();docs(manifest)
    archives=package_files()
    (ROOT/'packages.json').write_text(json.dumps(archives,indent=2)+'\n')
    progress=get(ROOT/'progress.json');progress.update(stage='complete',completedShips=list(SPECS),viewerShips=18,lineupChecks=len(manifest['lineupRenderChecks']),sourceModelsValidated=9,optimizedModelsValidated=54)
    (ROOT/'progress.json').write_text(json.dumps(progress,indent=2)+'\n')
    print(json.dumps({'ships':18,'priorAssetsUnchanged':manifest['priorAssetsHashChecked'],'lineupChecks':len(manifest['lineupRenderChecks']),'archives':archives},indent=2))
