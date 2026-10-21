import {createRequire} from 'node:module';
import {readFile, writeFile} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const root = path.dirname(fileURLToPath(import.meta.url));
const require = createRequire(import.meta.url);
let esbuild;
try { esbuild = require('esbuild'); }
catch { esbuild = createRequire(new URL('../pirate-fleet-v1/package.json', import.meta.url))('esbuild'); }
const result = await esbuild.build({entryPoints: [path.join(root, 'viewer.js')], bundle: true,
  minify: true, format: 'esm', write: false});
const model = await readFile(path.join(root, 'free-trader.glb'));
const info = JSON.parse(await readFile(path.join(root, 'model-info.json'), 'utf8'));
const template = await readFile(path.join(root, 'viewer-shell.html'), 'utf8');
const html = template.replace('__TRIANGLES__', info.triangles.toLocaleString('en-US'))
  .replace('__GLB_BASE64__', model.toString('base64'))
  .replace('__VIEWER_BUNDLE__', () => result.outputFiles[0].text.replaceAll('</script', '<\\/script'));
await writeFile(path.join(root, 'viewer.html'), html);
console.log('Built standalone viewer.html with embedded GLB and renderer.');
