import fs from 'node:fs';
import path from 'node:path';

const targetDir = path.resolve('tmp');
const targetFile = path.join(targetDir, 'tauri.release.conf.json');

fs.mkdirSync(targetDir, { recursive: true });

const config = {
  bundle: {
    createUpdaterArtifacts: true
  }
};

fs.writeFileSync(targetFile, `${JSON.stringify(config, null, 2)}\n`, 'utf8');
console.log(`Generated ${targetFile}`);
