// Bundle the established fleet inspector with each self-contained trader GLB.
import {createRequire} from 'node:module';
import {readFile, writeFile} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const root=path.dirname(fileURLToPath(import.meta.url));
const require=createRequire(new URL('../pirate-fleet-v1/package.json',import.meta.url));
const esbuild=require('esbuild');
const templateRoot=path.resolve(root,'../free-trader-textured-v1');
const source=(await readFile(path.join(templateRoot,'viewer.js'),'utf8'))
  .replace("const side = currentView === 'side';","const side = ['side', 'starboard'].includes(currentView);")
  .replace('side: [-1, 0, 0]','side: [-1, 0, 0], starboard: [1, 0, 0]')
  .replace("['top', 'bottom', 'front', 'back', 'side'].includes(requested)","['top', 'bottom', 'front', 'back', 'side', 'starboard'].includes(requested)")
  .replace('scene.add(model);',`scene.add(model);
  model.traverse(obj => {
    if (!obj.isMesh) return;
    for (const material of (Array.isArray(obj.material) ? obj.material : [obj.material])) {
      for (const key of ['map','metalnessMap','roughnessMap','emissiveMap']) {
        if (material[key]) material[key].anisotropy = Math.min(8, renderer.capabilities.getMaxAnisotropy());
      }
    }
  });`);
const shell=(await readFile(path.join(templateRoot,'viewer-shell.html'),'utf8'))
  .replace('<button data-view="side">Side</button>','<button data-view="side">Port</button><button data-view="starboard">Starboard</button>');
const ships=[
  ['clean-slate','Ines Varga','Enclosed precision freighter · ivory and teal'],
  ['second-wind','Mags Halloran','Salvage workboat · cargo cradle and repair crane'],
  ['small-fortune','Rafi Mensah','Fast trading runner · rust red and cream'],
];
const cards=[];
for(const [slug,owner,description] of ships){
  const folder=path.resolve(root,`../${slug}-textured-v1`);
  const info=JSON.parse(await readFile(path.join(folder,'model-info.json'),'utf8'));
  const model=await readFile(path.join(folder,`${slug}.glb`));
  const result=await esbuild.build({stdin:{contents:source.replaceAll('free-trader',slug),
    resolveDir:templateRoot,loader:'js'},bundle:true,minify:true,format:'esm',write:false});
  const links=ships.map(([id,name])=>`<a href="../${id}-textured-v1/viewer.html"${id===slug?' aria-current="page"':''}>${name.split(' ')[0]}</a>`).join('');
  const html=shell.replaceAll('Free Trader',info.name).replaceAll('FREE TRADER',info.name.toUpperCase())
    .replace('Purple &amp; cream livery · polished brass fittings',`${owner} · ${description}`)
    .replace('6 attachment sockets',`${info.sockets.length} attachment sockets`)
    .replace('__TRIANGLES__',info.triangles.toLocaleString('en-US'))
    .replace('</style>',`.fleetlinks{display:flex;gap:14px;margin-top:14px}.fleetlinks a{color:#969eaf;text-decoration:none;font-size:12px}.fleetlinks a:hover,.fleetlinks a[aria-current]{color:#f0e4ce}.fleetlinks .reference{margin-left:10px;color:#79b2bf}</style>`)
    .replace('</div><nav aria-label="Camera views">',`<div class="fleetlinks">${links}<a class="reference" href="textures/reference-sheet.png" target="_blank">2D reference</a></div></div><nav aria-label="Camera views">`)
    .replace('__GLB_BASE64__',model.toString('base64'))
    .replace('__VIEWER_BUNDLE__',()=>result.outputFiles[0].text.replaceAll('</script','<\\/script'));
  await writeFile(path.join(folder,'viewer.html'),html);
  cards.push(`<article><a class="preview" href="../${slug}-textured-v1/viewer.html"><img src="../${slug}-textured-v1/previews/three-quarter.png" alt="${info.name} rendered from its 3D model"></a><div class="details"><small>${owner}</small><h2>${info.name}</h2><p>${description}</p><div class="links"><a href="../${slug}-textured-v1/viewer.html">Explore in 3D ↗</a><a href="../${slug}-textured-v1/${slug}.glb" download>GLB ↓</a></div></div></article>`);
  console.log(`Built ${slug}/viewer.html (${(Buffer.byteLength(html)/1024/1024).toFixed(1)} MiB, embedded model and renderer)`);
}
await writeFile(path.join(root,'index.html'),`<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Independent traders · Stellar Syndicates</title><style>*{box-sizing:border-box}body{margin:0;background:#0e141c;color:#e7e5de;font:16px system-ui,sans-serif}main{max-width:1600px;margin:auto;padding:56px 40px}header{margin-bottom:36px}small{color:#9baabd;font-size:12px;letter-spacing:.12em}h1{font-weight:500;font-size:38px;margin:10px 0}header p{color:#a8b4c1}.grid{display:grid;grid-template-columns:repeat(3,1fr);gap:22px}article{background:#151e29;border:1px solid #293443;border-radius:14px;overflow:hidden}img{display:block;width:100%;aspect-ratio:1.18;object-fit:contain;background:#0b1015}.details{padding:25px}h2{font-size:26px;font-weight:500;margin:10px 0}p{font-size:14px;color:#b4beca;line-height:1.6}.links{display:flex;gap:24px;margin-top:22px}a{color:#94d4db;text-decoration:none}a:hover{color:#e2fdff}@media(max-width:900px){.grid{grid-template-columns:1fr}main{padding:30px 20px}}</style><main><header><small>STELLAR SYNDICATES / SHIP DESIGN</small><h1>Independent traders</h1><p>Three captains. Three ships. Open a model to orbit it, inspect its underside, or compare it with the concept.</p></header><section class="grid">${cards.join('')}</section></main></html>`);
