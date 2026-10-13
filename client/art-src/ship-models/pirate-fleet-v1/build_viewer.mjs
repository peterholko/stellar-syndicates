import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';

const root = path.dirname(fileURLToPath(import.meta.url));
const specifications = [
  ['privateer-raider', 'Privateer Raider', 'Salvaged armor, four forward barrels and three recessed engines.'],
  ['pirate-corsair', 'Pirate Corsair', 'A narrow needle hull with swept, hooked wings and patched hazard paint.'],
  ['pirate-boarding-raider', 'Pirate Boarding Raider', 'Layered armor, an offset bridge, a ventral airlock and open boarding cages.'],
];
const fleet = [];
for (const [id, title, description] of specifications) {
  const folder = path.join(root, '..', `${id}-textured-v1`);
  const readBase64 = async filename => (await fs.readFile(filename)).toString('base64');
  fleet.push({ id, title, description,
    info: JSON.parse(await fs.readFile(path.join(folder, 'model-info.json'), 'utf8')),
    glb: await readBase64(path.join(folder, `${id}.glb`)),
    reference: 'data:image/png;base64,'+await readBase64(path.join(folder, 'textures/reference-sheet.png')),
    preview: 'data:image/png;base64,'+await readBase64(path.join(folder, 'previews/hero.png')),
  });
}
const result = await build({
  entryPoints: [path.join(root, 'viewer.js')], bundle: true, minify: true,
  format: 'iife', platform: 'browser', target: ['es2020'], write: false,
  legalComments: 'inline',
});
const template = await fs.readFile(path.join(root, 'viewer-template.html'), 'utf8');
const output = template.replace('__FLEET_DATA__', JSON.stringify(fleet).replaceAll('<', '\\u003c'))
  .replace('__VIEWER_BUNDLE__', () => result.outputFiles[0].text.replaceAll('</script', '<\\/script'));
await fs.writeFile(path.join(root, 'viewer.html'), output);
console.log(`Offline viewer built: ${(Buffer.byteLength(output)/1048576).toFixed(1)} MB; all models, references, previews and Three.js embedded.`);
