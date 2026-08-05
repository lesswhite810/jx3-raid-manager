import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, test, vi } from 'vitest';
import {
  bumpPatchVersion,
  buildReleaseNotesTemplate,
  isVersionPublished,
  prepareNextVersion,
} from './prepare-next-version.mjs';

const tempDirs = [];

function createFixture({ version = '2.1.18', published = true } = {}) {
  const rootDir = fs.mkdtempSync(path.join(os.tmpdir(), 'jx3-version-test-'));
  tempDirs.push(rootDir);

  fs.mkdirSync(path.join(rootDir, 'src-tauri'), { recursive: true });
  fs.mkdirSync(path.join(rootDir, 'release-notes'), { recursive: true });

  fs.writeFileSync(
    path.join(rootDir, 'package.json'),
    `${JSON.stringify({ name: 'jx3-raid-manager', version }, null, 2)}\n`,
    'utf8'
  );
  fs.writeFileSync(
    path.join(rootDir, 'package-lock.json'),
    `${JSON.stringify(
      {
        name: 'jx3-raid-manager',
        version,
        packages: {
          '': {
            version,
          },
        },
      },
      null,
      2
    )}\n`,
    'utf8'
  );
  fs.writeFileSync(
    path.join(rootDir, 'src-tauri', 'tauri.conf.json'),
    `${JSON.stringify({ version }, null, 2)}\n`,
    'utf8'
  );
  fs.writeFileSync(
    path.join(rootDir, 'src-tauri', 'Cargo.toml'),
    `[package]\nname = "jx3-raid-manager"\nversion = "${version}"\n`,
    'utf8'
  );
  fs.writeFileSync(
    path.join(rootDir, 'src-tauri', 'Cargo.lock'),
    `[[package]]\nname = "jx3-raid-manager"\nversion = "${version}"\n`,
    'utf8'
  );

  // 初始化 git 仓库并打 tag（模拟已发布状态）
  if (published) {
    const { execFileSync } = require('node:child_process');
    execFileSync('git', ['init', '-q'], { cwd: rootDir });
    execFileSync('git', ['config', 'user.email', 'test@test.com'], { cwd: rootDir });
    execFileSync('git', ['config', 'user.name', 'Test'], { cwd: rootDir });
    execFileSync('git', ['add', '-A'], { cwd: rootDir });
    execFileSync('git', ['commit', '-q', '-m', 'init'], { cwd: rootDir });
    execFileSync('git', ['tag', `v${version}`], { cwd: rootDir });
  }

  return rootDir;
}

afterEach(() => {
  vi.restoreAllMocks();
  for (const dir of tempDirs.splice(0)) {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

describe('bumpPatchVersion', () => {
  test('calculates the next patch version', () => {
    expect(bumpPatchVersion('2.1.18')).toBe('2.1.19');
  });
});

describe('prepareNextVersion', () => {
  test('syncs version files and creates a release notes template', () => {
    const rootDir = createFixture();

    const result = prepareNextVersion(['2.1.19'], rootDir);

    expect(result.version).toBe('2.1.19');
    expect(JSON.parse(fs.readFileSync(path.join(rootDir, 'package.json'), 'utf8')).version).toBe('2.1.19');
    expect(JSON.parse(fs.readFileSync(path.join(rootDir, 'package-lock.json'), 'utf8')).version).toBe('2.1.19');
    expect(
      JSON.parse(fs.readFileSync(path.join(rootDir, 'package-lock.json'), 'utf8')).packages[''].version
    ).toBe('2.1.19');
    expect(JSON.parse(fs.readFileSync(path.join(rootDir, 'src-tauri', 'tauri.conf.json'), 'utf8')).version).toBe(
      '2.1.19'
    );
    expect(fs.readFileSync(path.join(rootDir, 'src-tauri', 'Cargo.toml'), 'utf8')).toContain(
      'version = "2.1.19"'
    );
    expect(fs.readFileSync(path.join(rootDir, 'src-tauri', 'Cargo.lock'), 'utf8')).toContain(
      'version = "2.1.19"'
    );
    expect(fs.readFileSync(path.join(rootDir, 'release-notes', 'v2.1.19.md'), 'utf8')).toBe(
      buildReleaseNotesTemplate('2.1.19')
    );
  });

  test('supports preparing the next patch directly from a release version', () => {
    const rootDir = createFixture();

    const result = prepareNextVersion(['--next-patch-from', '2.1.19'], rootDir);

    expect(result.version).toBe('2.1.20');
    expect(fs.existsSync(path.join(rootDir, 'release-notes', 'v2.1.20.md'))).toBe(true);
  });

  test('refuses to bump when current version is not published', () => {
    const rootDir = createFixture({ version: '2.1.50', published: false });

    expect(() => prepareNextVersion(['2.1.51'], rootDir)).toThrow(/禁止推高版本号.*2.1.50.*尚未发布/);
  });

  test('--force bypasses the unpublished check', () => {
    const rootDir = createFixture({ version: '2.1.50', published: false });

    const result = prepareNextVersion(['2.1.51', '--force'], rootDir);

    expect(result.version).toBe('2.1.51');
    expect(JSON.parse(fs.readFileSync(path.join(rootDir, 'package.json'), 'utf8')).version).toBe('2.1.51');
  });
});

describe('isVersionPublished', () => {
  test('returns true when local tag exists', () => {
    const rootDir = createFixture({ version: '2.1.30' });
    expect(isVersionPublished('2.1.30', rootDir)).toBe(true);
  });

  test('returns false when tag does not exist', () => {
    const rootDir = createFixture({ version: '2.1.30' });
    expect(isVersionPublished('2.1.31', rootDir)).toBe(false);
  });

  test('returns false for invalid version format', () => {
    const rootDir = createFixture({ version: '2.1.30' });
    expect(isVersionPublished('invalid', rootDir)).toBe(false);
  });
});
