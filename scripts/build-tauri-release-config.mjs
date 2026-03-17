import fs from 'node:fs';
import path from 'node:path';

const targetDir = path.resolve('tmp');
const targetFile = path.join(targetDir, 'tauri.release.conf.json');
const sourceFile = path.resolve('src-tauri', 'tauri.conf.json');
const githubUpdaterEndpoint =
  'https://github.com/lesswhite810/jx3-raid-manager/releases/latest/download/latest.json';
const giteeRepo = process.env.GITEE_REPO ?? 'lesswhite810/jx3-raid-manager';
const giteeAssetsBranch = process.env.GITEE_ASSETS_BRANCH ?? 'updater-assets';
const giteeUpdaterEndpoint =
  `https://gitee.com/${giteeRepo}/raw/${giteeAssetsBranch}/updater/latest.json`;
const publicKey = (process.env.TAURI_PUBLIC_KEY ?? '').trim();

fs.mkdirSync(targetDir, { recursive: true });

const baseConfig = JSON.parse(fs.readFileSync(sourceFile, 'utf8'));
const config = {
  ...baseConfig,
  bundle: {
    ...(baseConfig.bundle ?? {}),
    createUpdaterArtifacts: true
  },
  plugins: {
    ...(baseConfig.plugins ?? {}),
    updater: {
      ...(baseConfig.plugins?.updater ?? {}),
      endpoints: [githubUpdaterEndpoint, giteeUpdaterEndpoint],
      pubkey: publicKey
    }
  }
};

fs.writeFileSync(targetFile, `${JSON.stringify(config, null, 2)}\n`, 'utf8');
console.log(`Generated ${targetFile}`);
