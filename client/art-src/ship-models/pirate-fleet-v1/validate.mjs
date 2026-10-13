import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import validator from 'gltf-validator';

const root = path.dirname(fileURLToPath(import.meta.url));
const ships = ['privateer-raider', 'pirate-corsair', 'pirate-boarding-raider'];
let failed = false;
for (const ship of ships) {
  const folder = path.join(root, '..', `${ship}-textured-v1`);
  const bytes = await fs.readFile(path.join(folder, `${ship}.glb`));
  const report = await validator.validateBytes(new Uint8Array(bytes), {
    uri: `${ship}.glb`, maxIssues: 1000,
  });
  await fs.writeFile(path.join(folder, 'gltf-validation.json'), JSON.stringify(report, null, 2)+'\n');
  console.log(`${ship}: ${report.issues.numErrors} errors, ${report.issues.numWarnings} warnings, ${bytes.length.toLocaleString()} bytes`);
  if (report.issues.numErrors || report.issues.numWarnings) {
    failed = true;
    console.log(report.issues.messages);
  }
}
if (failed) process.exitCode = 1;
