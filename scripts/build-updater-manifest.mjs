import fs from 'node:fs';
import path from 'node:path';

const [tag, channel = 'github', customOutputPath] = process.argv.slice(2);

if (!tag) {
  throw new Error('Missing release tag. Usage: node scripts/build-updater-manifest.mjs v2.1.12 [github|gitee] [outputPath]');
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
const outputPath = customOutputPath
  ? path.resolve(customOutputPath)
  : path.resolve('src-tauri', 'target', 'release', 'bundle', channel === 'gitee' ? 'latest.gitee.json' : 'latest.json');
const giteeRepo = process.env.GITEE_REPO ?? 'lesswhite810/jx3-raid-manager';
const giteeAssetsBranch = process.env.GITEE_ASSETS_BRANCH ?? 'updater-assets';

const buildPlatformUrl = () => {
  if (channel === 'gitee') {
    return `https://gitee.com/${giteeRepo}/raw/${giteeAssetsBranch}/updater/JX3RaidManager_${version}_x64-setup.exe`;
  }
  return `https://github.com/lesswhite810/jx3-raid-manager/releases/download/${tag}/JX3RaidManager_${version}_x64-setup.exe`;
};

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
      url: buildPlatformUrl()
    }
  }
};

fs.mkdirSync(path.dirname(outputPath), { recursive: true });
fs.writeFileSync(outputPath, `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');
console.log(`Generated ${outputPath}`);
