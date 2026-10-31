import { createRequire } from 'node:module';
import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = path.dirname(fileURLToPath(import.meta.url));
const require = createRequire(new URL('../interceptor-lod-experiment-v1/validate.mjs', import.meta.url));
const validator = require('gltf-validator');
const reports = {};
for (const level of ['source', 'lod0', 'lod1', 'lod2']) {
  const file = level === 'source' ? 'combat-freighter.glb'
    : `../interceptor-lod-experiment-v1/models/combat-freighter-${level}.glb`;
  const report = await validator.validateBytes(new Uint8Array(await readFile(path.join(root, file))), { uri: file });
  reports[level] = report;
  const { numErrors, numWarnings } = report.issues;
  console.log(`${level}: ${numErrors} errors, ${numWarnings} warnings`);
  if (numErrors || numWarnings) process.exitCode = 1;
}
await writeFile(path.join(root, 'gltf-validation.json'), JSON.stringify(reports, null, 2) + '\n');
