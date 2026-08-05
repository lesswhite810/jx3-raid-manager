import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
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

/**
 * 检查指定版本是否已发布（存在 git tag v<version>）。
 *
 * 用于防止本地在版本未发布时手动推高版本号造成"占位版本"。
 * 检查策略：
 * 1. 优先检查 git 本地 tag（git rev-parse v<version>）。
 * 2. 如果本地不存在，再调用 git ls-remote 检查远程 tag。
 *
 * @param {string} version - 待检查的版本号
 * @param {string} rootDir - 仓库根目录
 * @returns {boolean} 是否已发布
 */
export function isVersionPublished(version, rootDir = process.cwd()) {
  if (!isValidVersion(version)) {
    return false;
  }
  const tag = `v${version}`;
  try {
    execFileSync('git', ['rev-parse', tag], { cwd: rootDir, stdio: 'ignore' });
    return true;
  } catch {
    // 本地 tag 不存在，继续检查远程
  }
  try {
    const output = execFileSync('git', ['ls-remote', '--tags', 'origin', `refs/tags/${tag}`], {
      cwd: rootDir,
      encoding: 'utf8',
    }).trim();
    return output.length > 0;
  } catch {
    // 无法访问远程（无网络/无 origin），保守起见视为未发布
    return false;
  }
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
  let force = false;

  for (let index = 0; index < args.length; index += 1) {
    const value = args[index];
    if (value === '--next-patch-from') {
      nextPatchFrom = args[index + 1] ?? null;
      index += 1;
      continue;
    }
    if (value === '--force' || value === '-f') {
      force = true;
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
    throw new Error(
      'Usage: node scripts/prepare-next-version.mjs <version> | --next-patch-from <version> [--force]'
    );
  }

  return { version: resolvedVersion, force };
}

/**
 * 准备下一个版本的发布配置（同步版本号 + 生成 release notes 模板）。
 *
 * 强制要求：当前仓库的版本号必须已经发布（存在 git tag v<currentVersion>），
 * 否则禁止推高版本号，避免出现"占位版本"。
 *
 * 例外：使用 `--force` 标志可绕过此检查，用于补救历史遗留的版本状态。
 *
 * @param {string[]} args - 命令行参数
 * @param {string} rootDir - 仓库根目录
 */
export function prepareNextVersion(args, rootDir = process.cwd()) {
  const { version, force } = parseArgs(args);

  if (!force) {
    const packageJsonPath = path.join(rootDir, 'package.json');
    const currentVersion = readJson(packageJsonPath).version;
    if (currentVersion && isValidVersion(currentVersion) && !isVersionPublished(currentVersion, rootDir)) {
      throw new Error(
        `禁止推高版本号：当前仓库版本 ${currentVersion} 尚未发布（git tag v${currentVersion} 不存在）。\n` +
          `如需继续推高，请先在 GitHub 上发布当前版本，或使用 --force 标志绕过此检查。`
      );
    }
  }

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
