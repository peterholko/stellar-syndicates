import sharp from 'sharp';
import { mkdir, copyFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../', import.meta.url));
const source = root + 'art-src/exploration/';
const target = root + 'public/art/exploration/';
await mkdir(target, { recursive: true });
const inputs = {
  derelict: 'exec-3032e5a3-99b8-4abc-b2da-1f0f84025b9b.png',
  station: 'exec-0de171be-4b8e-4829-9b8e-57c769efc96f.png',
  asteroids: 'exec-097f6031-220e-4e68-8ba7-c85871e4398b.png',
  anomaly: 'exec-8666bc7b-f166-4ecb-9dd3-e56e4f35af2f.png',
  precursor: 'exec-9250b4b2-0ba1-483c-86a0-97aba5c02786.png',
};
for (const [kind, original] of Object.entries(inputs)) {
  if (process.argv[2]) await copyFile(process.argv[2] + '/' + original, source + kind + '.png');
  const meta = await sharp(source + kind + '.png').metadata();
  if (!meta.hasAlpha) throw new Error(kind + ': expected transparent source');
  await sharp(source + kind + '.png').resize(256, 256, { fit: 'inside', kernel: 'lanczos3' })
    .png({ compressionLevel: 9 }).toFile(target + kind + '.png');
  console.log(kind + ': transparent 256px runtime sprite');
}
