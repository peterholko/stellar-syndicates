"""Extend the existing lab without rebuilding or changing its first nine assets."""
from pathlib import Path
import json,re
from scaffold import SPECS,MODELS
lab=MODELS/'interceptor-lod-experiment-v1'
old=['interceptor','corvette','cruiser','tiny-freighter','small-freighter','large-freighter','heavy-freighter','bulk-freighter','titan']
new=list(SPECS);allids=old+new

def edit(name,fn):
    file=lab/name;file.write_text(fn(file.read_text()))

jsids=json.dumps(allids)
for name in ('build_preview.mjs','validate.mjs'):
    edit(name,lambda s:re.sub(r"\['interceptor', 'corvette', 'cruiser', 'tiny-freighter', 'small-freighter', 'large-freighter', 'heavy-freighter', 'bulk-freighter', 'titan'\]",jsids,s))
edit('preview_server.py',lambda s:s.replace('("interceptor", "corvette", "cruiser", "tiny-freighter", "small-freighter", "large-freighter", "heavy-freighter", "bulk-freighter", "titan")',str(tuple(allids))))

def models(s):
    s=s.replace("titan: 'Titan' };","titan: 'Titan', "+', '.join(json.dumps(k)+': '+json.dumps(v['title']) for k,v in SPECS.items())+' };')
    s=s.replace("const title = titles[ship];", "const title = titles[ship];\nconst newShips = "+json.dumps(new)+";\nlet modelSpec = null;\nif (newShips.includes(ship)) modelSpec = JSON.parse(await readFile(path.join(root, `../${ship}-textured-v1/model-spec.json`), 'utf8'));" )
    s=s.replace("function protectedComponent(name) {", "function protectedComponent(name) {\n  if (newShips.includes(ship)) return /^(hull[ .]|armor[ .]|keel[ .]|spine[ .]|bridge[ .]|cargo[ .]|structure[ .])/.test(name) || /armored turret housing/.test(name);")
    s=s.replace("const assembly = ['titan', 'corvette'].includes(ship) ? mesh.extras.assembly : ship === 'cruiser' ? mesh.name.match(/^turret\\.(port|starboard)/)?.[0] ?? 'hull' : 'hull';", "const assembly = mesh.extras?.assembly ?? (ship === 'cruiser' ? mesh.name.match(/^turret\\.(port|starboard)/)?.[0] ?? 'hull' : 'hull');")
    s=re.sub(r"const turretAssemblies = .*?;\n", "const turretAssemblies = sockets.filter(n => n.name.startsWith('socket.turret.')).map(n => n.name.slice('socket.turret.'.length));\n",s)
    s=s.replace("protectedFeatures: ship === 'corvette'", "referenceRegions: modelSpec?.regions,\n  protectedFeatures: newShips.includes(ship) ? ['Primary closed hull contours and structural modules', 'Command cabin, cargo cages and attachment frames', 'Dedicated bottom-view projection and ventral plating', 'Independent armored turret housings, where present'] : ship === 'corvette'")
    return s
edit('build_models.mjs',models)

def verification(s):
    s=s.replace("('interceptor', 'corvette', 'cruiser', 'tiny-freighter', 'small-freighter', 'large-freighter', 'heavy-freighter', 'bulk-freighter', 'titan')",str(tuple(allids)))
    s=s.replace('def protected_component(name):','NEW_SHIPS='+repr(tuple(new))+"\nNEW_SPEC=json.loads((ROOT.parent/(SHIP+'-textured-v1')/'model-spec.json').read_text()) if SHIP in NEW_SHIPS else None\n\ndef protected_component(name):\n    if SHIP in NEW_SHIPS:\n        return bool(re.match(r'^(hull[ .]|armor[ .]|keel[ .]|spine[ .]|bridge[ .]|cargo[ .]|structure[ .])',name) or 'armored turret housing' in name)")
    s=s.replace("SHIP.endswith('-freighter') or SHIP in ('titan', 'corvette')", "SHIP in NEW_SHIPS or SHIP.endswith('-freighter') or SHIP in ('titan', 'corvette')")
    s=s.replace('region=(532,72,987,799)',"region=NEW_SPEC['bottom'] if NEW_SPEC else (532,72,987,799)")
    s=s.replace("if SHIP in ('cruiser', 'titan', 'corvette'):","if info.get('articulations'):")
    s=s.replace("(16 if SHIP in ('titan', 'corvette') else 10)","sum(x['assembly'].startswith('turret.') for x in info['levels'][0]['detail'])")
    s=s.replace("(8 if SHIP in ('titan', 'corvette') else 4)","len(info['articulations'])*2")
    return s
edit('verify_geometry.py',verification)

def shell(s):
    s=s.replace('DETAIL & DISTANCE · NINE SHIPS','DETAIL & DISTANCE · 18 SHIPS')
    a=s.index('<select id="ship-type">');b=s.index('</select>',a)
    categories=[('Combat',['interceptor','corvette','destroyer','cruiser','battleship','dreadnought','titan']),
                ('Utility',['scout','colony-ship','troop-transport','construction-ship']),
                ('Freighters',['tiny-freighter','small-freighter','medium-freighter','large-freighter','heavy-freighter','bulk-freighter','authority-freighter'])]
    options='<select id="ship-type">'
    for category,ids in categories:
        options+='<optgroup label="'+category+'">'
        for ship in ids:options+='<option value="'+ship+'"'+(' selected' if ship=='cruiser' else '')+'>'+ship.replace('-',' ').title()+'</option>'
        options+='</optgroup>'
    return s[:a]+options+s[b:]
edit('viewer-shell.html',shell)

def viewer(s):
    s=s.replace("lod0: `Every original triangle, normal, and texture is preserved. ${", "lod0: `Every original triangle, normal, and texture is preserved. ${info.referenceRegions ? `${info.levels[0].drawCalls} render sections; ${info.articulations.length} independent turret mounts.` : ")
    s=s.replace("shipId === 'corvette' ? 'Forward shoulders", "info.referenceRegions ? 'Main hulls, structural frames, cargo modules, ventral plating, and turret housings stay intact; rounded fittings are simplified.' : shipId === 'corvette' ? 'Forward shoulders")
    s=s.replace("titleFor(ship.id).replace(' Freighter', '\\nFreighter')", "titleFor(ship.id).replace(' ', '\\n')")
    s=s.replace("ctx.fillText(shipId === 'corvette' ?", "ctx.fillText(info.referenceRegions ? `Original reference textures · ${info.articulations.length} independent turrets · dedicated underside` : shipId === 'corvette' ?")
    s=s.replace("singlePreservation = id === 'corvette' ?", "singlePreservation = info.referenceRegions ? `Reference textures cover all five sides. Lower levels preserve primary hulls, structural frames, cargo modules, and armored turret housings. ${info.articulations.length} independent turret mounts. Round fittings use fewer triangles; sprites include underside views.` : id === 'corvette' ?")
    a=s.index('  let x = 0;\n',s.index('function layoutComparison()'));b=s.index('  updatePresentation(); fit();',a)
    s=s[:a]+'''  const perRow = Math.ceil(ships.length / 2);
  for (let row = 0; row < 2; row++) {
    const members = ships.slice(row * perRow, (row + 1) * perRow);
    const sizes = members.map(ship => {
      const scale = comparisonScale === 'game' ? reference * displayLengthRatios[ship.id] / ship.model.size.z : 1;
      return { ship, scale, width: Math.max(ship.model.size.x * scale, maxLength * .25) };
    });
    const gap = maxLength * .12, rowWidth = sizes.reduce((n, s) => n + s.width, 0) + gap * (members.length - 1);
    const stern = (row - 1) * maxLength * 1.45;
    let x = -rowWidth / 2;
    for (const { ship, scale, width } of sizes) {
      ship.group.scale.setScalar(scale);
      ship.group.position.set(x + width / 2 - ship.model.center.x * scale, -ship.model.center.y * scale, stern - ship.model.bounds.max.z * scale);
      ship.labelAnchor = new THREE.Vector3(x + width / 2, 0, stern + maxLength * .055);
      ship.length = ship.model.size.z * scale;
      ship.lengthLabel = comparisonScale === 'game' ? `${displayLengthRatios[ship.id].toFixed(2)}×` : `${ship.length.toFixed(2)} units`;
      ship.label.querySelector('small').textContent = ship.lengthLabel;
      ship.label.dataset.length = ship.length;
      const bounds = ship.model.bounds.clone(); bounds.min.multiplyScalar(scale).add(ship.group.position); bounds.max.multiplyScalar(scale).add(ship.group.position);
      comparisonBounds.union(bounds); x += width + gap;
    }
  }
  comparisonBounds.max.z = maxLength * .24;
''' +s[b:]
    # Decode one ship at a time; avoid a peak of seventy-two simultaneous GLBs.
    s=s.replace('await Promise.all(Object.keys(catalog).map(loadShip));','for (const id of Object.keys(catalog)) await loadShip(id);')
    return s
edit('viewer.js',viewer)
edit('comparison-scale.js',lambda s:s.replace("titan: 8,","titan: 8,\n  destroyer: 2.5, battleship: 5, dreadnought: 6,\n  ...Object.fromEntries(Object.entries({scout:80, 'colony-ship':6000, 'troop-transport':7000, 'construction-ship':2500, 'medium-freighter':4500, 'authority-freighter':6000}).map(([id,mass]) => [id, Math.max(10,Math.min(64,7*(mass/100)**.4))/10])),"))
print('Integrated eighteen ship definitions and two-row size comparison.')
