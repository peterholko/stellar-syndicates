import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
const root = path.dirname(fileURLToPath(import.meta.url));
let require = createRequire(import.meta.url);
let validator;
try { validator = require('gltf-validator'); }
catch { require = createRequire(new URL('../pirate-fleet-v1/package.json', import.meta.url)); validator = require('gltf-validator'); }
const filename = path.resolve(root, process.argv[2] || '../mining-barge-textured-v1/mining-barge.glb');
const bytes = await fs.readFile(filename);
const result = await validator.validateBytes(new Uint8Array(bytes), { uri: path.basename(filename), maxIssues: 1000 });
await fs.writeFile(path.join(path.dirname(filename), 'gltf-validation.json'), JSON.stringify(result, null, 2)+'\n');
console.log(`${result.issues.numErrors} glTF errors; ${result.issues.numWarnings} warnings; ${bytes.length.toLocaleString()} bytes`);
if (result.issues.numErrors || result.issues.numWarnings) process.exitCode = 1;
