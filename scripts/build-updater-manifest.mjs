import fs from 'node:fs';
import path from 'node:path';

const [tag] = process.argv.slice(2);

if (!tag) {
  throw new Error('Missing release tag. Usage: node scripts/build-updater-manifest.mjs v2.1.12');
}

const version = tag.startsWith('v') ? tag.slice(1) : tag;
const releaseNotesPath = path.resolve('release-notes', `v${version}.md`);
const signaturePath = path.resolve(
  'src-tauri',
  'target',
  'release',
  'bundle',
  'nsis',
  `JX3RaidManager_${version}_x64-setup.exe.sig`
);
const outputPath = path.resolve('src-tauri', 'target', 'release', 'bundle', 'latest.json');

if (!fs.existsSync(releaseNotesPath)) {
  throw new Error(`Release notes not found: ${releaseNotesPath}`);
}

if (!fs.existsSync(signaturePath)) {
  throw new Error(`Signature not found: ${signaturePath}`);
}

const notes = fs.readFileSync(releaseNotesPath, 'utf8').trim();
const signature = fs.readFileSync(signaturePath, 'utf8').trim();
const manifest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    'windows-x86_64': {
      signature,
      url: `https://github.com/lesswhite810/jx3-raid-manager/releases/download/${tag}/JX3RaidManager_${version}_x64-setup.exe`
    }
  }
};

fs.writeFileSync(outputPath, `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');
console.log(`Generated ${outputPath}`);
