import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const root = path.dirname(fileURLToPath(import.meta.url));
let require = createRequire(import.meta.url);
let esbuild;
try { esbuild = require('esbuild'); }
catch { require = createRequire(new URL('../pirate-fleet-v1/package.json', import.meta.url)); esbuild = require('esbuild'); }
const base64 = async name => (await fs.readFile(path.join(root, name))).toString('base64');
const trafficManifest = JSON.parse(await fs.readFile(path.join(root, 'traffic-models/manifest.json'), 'utf8'));
const asset = {
  glb: await base64('authority-nexus.glb'),
  reference: 'data:image/png;base64,'+await base64('reference.png'),
  preview: 'data:image/png;base64,'+await base64('previews/hero.png'),
  info: JSON.parse(await fs.readFile(path.join(root, 'model-info.json'), 'utf8')),
  disabled: {
    glb: await base64('disabled/authority-nexus-disabled.glb'),
    preview: 'data:image/png;base64,'+await base64('disabled/previews/hero.png'),
    info: JSON.parse(await fs.readFile(path.join(root, 'disabled/model-info.json'), 'utf8')),
  },
  traffic: Object.fromEntries(await Promise.all(trafficManifest.ships.map(async ship => [ship.id, await base64(ship.file)]))),
};
const result = await esbuild.build({
  entryPoints: [path.join(root, 'viewer.js')], bundle: true, minify: true,
  format: 'iife', platform: 'browser', target: ['es2020'], write: false,
  nodePaths: [path.dirname(path.dirname(path.dirname(require.resolve('three'))))],
  legalComments: 'inline',
});
const template = await fs.readFile(path.join(root, 'viewer-template.html'), 'utf8');
const html = template.replace('__ASSET_DATA__', JSON.stringify(asset).replaceAll('<', '\\u003c'))
  .replace('__VIEWER_BUNDLE__', () => result.outputFiles[0].text.replaceAll('</script', '<\\/script'));
await fs.writeFile(path.join(root, 'viewer.html'), html);
await fs.copyFile(path.join(path.dirname(path.dirname(require.resolve('three'))), 'LICENSE'), path.join(root, 'THREE-LICENSE.txt'));
console.log(`Saved offline viewer: ${(Buffer.byteLength(html)/1048576).toFixed(1)} MB`);
