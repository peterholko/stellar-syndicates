import {createRequire} from 'node:module';
import {readFile,writeFile} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const root=path.dirname(fileURLToPath(import.meta.url));
let require=createRequire(import.meta.url);
let esbuild;
let dependencies=path.join(root,'node_modules');
try {esbuild=require('esbuild');}
catch {require=createRequire(new URL('../pirate-fleet-v1/package.json',import.meta.url));esbuild=require('esbuild');dependencies=path.join(root,'../pirate-fleet-v1/node_modules');}
const result=await esbuild.build({entryPoints:[path.join(root,'viewer.js')],bundle:true,minify:true,format:'esm',write:false,nodePaths:[dependencies]});
const model=await readFile(path.join(root,'authority-arbiter.glb'));
const darkModel=await readFile(path.join(root,'authority-arbiter-dark.glb'));
const info=JSON.parse(await readFile(path.join(root,'model-info.json'),'utf8'));
const template=await readFile(path.join(root,'viewer-shell.html'),'utf8');
const html=template.replace('__TRIANGLES__',info.triangles.toLocaleString('en-US'))
  .replace('__RENDER_SECTIONS__',String(info.renderSections))
  .replace('__GLB_BASE64__',model.toString('base64'))
  .replace('__DARK_GLB_BASE64__',darkModel.toString('base64'))
  .replace('__VIEWER_BUNDLE__',()=>result.outputFiles[0].text.replaceAll('</script','<\\/script'));
await writeFile(path.join(root,'viewer.html'),html);
console.log('Built self-contained Authority Arbiter viewer with ivory/dark variants and galaxy-background controls.');
