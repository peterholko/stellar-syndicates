import {createRequire} from 'node:module';
import {readFile,writeFile} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const root=path.dirname(fileURLToPath(import.meta.url));
const require=createRequire(new URL('../pirate-fleet-v1/package.json',import.meta.url));
const esbuild=require('esbuild');
const template=path.resolve(root,'../free-trader-textured-v1');
const info=JSON.parse(await readFile(path.join(root,'model-info.json'),'utf8'));
let source=(await readFile(path.join(template,'viewer.js'),'utf8')).replaceAll('free-trader','combat-freighter');
source=source.replace('scene.add(model);',`scene.add(model);model.traverse(obj=>{if(obj.isMesh)for(const m of (Array.isArray(obj.material)?obj.material:[obj.material]))for(const key of ['map','roughnessMap','metalnessMap','emissiveMap'])if(m[key])m[key].anisotropy=Math.min(8,renderer.capabilities.getMaxAnisotropy());});`);
const result=await esbuild.build({stdin:{contents:source,resolveDir:template,loader:'js'},bundle:true,minify:true,format:'esm',write:false});
const bytes=await readFile(path.join(root,'combat-freighter.glb'));
let html=(await readFile(path.join(template,'viewer-shell.html'),'utf8'))
 .replaceAll('Free Trader','Combat Freighter').replaceAll('FREE TRADER','COMBAT FREIGHTER')
 .replace('INDEPENDENT MERCHANT','RAIDING CARGO CARRIER')
 .replace('Purple &amp; cream livery · polished brass fittings','Armored cargo citadel · four prize clamps · three engines · unarmed')
 .replace('6 attachment sockets','3 engine sockets')
 .replace('__TRIANGLES__',info.triangles.toLocaleString('en-US'))
 .replace('__GLB_BASE64__',bytes.toString('base64'))
 .replace('__VIEWER_BUNDLE__',()=>result.outputFiles[0].text.replaceAll('</script','<\\/script'));
await writeFile(path.join(root,'viewer.html'),html);
console.log('Built self-contained Combat Freighter viewer');
