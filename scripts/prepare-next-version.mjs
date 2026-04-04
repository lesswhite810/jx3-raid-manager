import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const VERSION_PATTERN = /^\d+\.\d+\.\d+$/;

export function isValidVersion(version) {
  return VERSION_PATTERN.test(version);
}

export function bumpPatchVersion(version) {
  if (!isValidVersion(version)) {
    throw new Error(`Invalid version: ${version}`);
  }

  const [major, minor, patch] = version.split('.').map(Number);
  return `${major}.${minor}.${patch + 1}`;
}

export function buildReleaseNotesTemplate(version) {
  return `# 剑网三副本管家 v${version}

## 新增需求

## 修复 bug
`;
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

function writeJson(filePath, value) {
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}

function updateCargoVersion(content, version) {
  return content.replace(/^version\s*=\s*"[^"]+"$/m, `version = "${version}"`);
}

function updateCargoLockVersion(content, version) {
  return content.replace(
    /(\[\[package\]\]\r?\nname = "jx3-raid-manager"\r?\nversion = )"[^"]+"/,
    `$1"${version}"`
  );
}

export function syncVersionFiles(version, rootDir = process.cwd()) {
  if (!isValidVersion(version)) {
    throw new Error(`Invalid version: ${version}`);
  }

  const packageJsonPath = path.join(rootDir, 'package.json');
  const packageLockPath = path.join(rootDir, 'package-lock.json');
  const tauriConfigPath = path.join(rootDir, 'src-tauri', 'tauri.conf.json');
  const cargoTomlPath = path.join(rootDir, 'src-tauri', 'Cargo.toml');
  const cargoLockPath = path.join(rootDir, 'src-tauri', 'Cargo.lock');

  const packageJson = readJson(packageJsonPath);
  packageJson.version = version;
  writeJson(packageJsonPath, packageJson);

  const packageLock = readJson(packageLockPath);
  packageLock.version = version;
  if (packageLock.packages?.['']) {
    packageLock.packages[''].version = version;
  }
  writeJson(packageLockPath, packageLock);

  const tauriConfig = readJson(tauriConfigPath);
  tauriConfig.version = version;
  writeJson(tauriConfigPath, tauriConfig);

  const cargoToml = fs.readFileSync(cargoTomlPath, 'utf8');
  fs.writeFileSync(cargoTomlPath, updateCargoVersion(cargoToml, version), 'utf8');

  if (fs.existsSync(cargoLockPath)) {
    const cargoLock = fs.readFileSync(cargoLockPath, 'utf8');
    fs.writeFileSync(cargoLockPath, updateCargoLockVersion(cargoLock, version), 'utf8');
  }
}

export function ensureReleaseNotesTemplate(version, rootDir = process.cwd()) {
  const notesPath = path.join(rootDir, 'release-notes', `v${version}.md`);
  if (!fs.existsSync(notesPath)) {
    fs.writeFileSync(notesPath, buildReleaseNotesTemplate(version), 'utf8');
  }
  return notesPath;
}

function parseArgs(args) {
  let explicitVersion = null;
  let nextPatchFrom = null;

  for (let index = 0; index < args.length; index += 1) {
    const value = args[index];
    if (value === '--next-patch-from') {
      nextPatchFrom = args[index + 1] ?? null;
      index += 1;
      continue;
    }

    if (!explicitVersion) {
      explicitVersion = value;
      continue;
    }

    throw new Error(`Unknown argument: ${value}`);
  }

  if (explicitVersion && nextPatchFrom) {
    throw new Error('Pass either an explicit version or --next-patch-from, not both.');
  }

  const resolvedVersion = explicitVersion ?? (nextPatchFrom ? bumpPatchVersion(nextPatchFrom) : null);
  if (!resolvedVersion) {
    throw new Error('Usage: node scripts/prepare-next-version.mjs <version> | --next-patch-from <version>');
  }

  return resolvedVersion;
}

export function prepareNextVersion(args, rootDir = process.cwd()) {
  const version = parseArgs(args);
  syncVersionFiles(version, rootDir);
  const notesPath = ensureReleaseNotesTemplate(version, rootDir);
  return { version, notesPath };
}

function main() {
  const { version, notesPath } = prepareNextVersion(process.argv.slice(2));
  process.stdout.write(`Prepared version ${version} and ${path.relative(process.cwd(), notesPath)}\n`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
