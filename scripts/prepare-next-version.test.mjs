import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, test } from 'vitest';
import {
  bumpPatchVersion,
  buildReleaseNotesTemplate,
  prepareNextVersion,
} from './prepare-next-version.mjs';

const tempDirs = [];

function createFixture() {
  const rootDir = fs.mkdtempSync(path.join(os.tmpdir(), 'jx3-version-test-'));
  tempDirs.push(rootDir);

  fs.mkdirSync(path.join(rootDir, 'src-tauri'), { recursive: true });
  fs.mkdirSync(path.join(rootDir, 'release-notes'), { recursive: true });

  fs.writeFileSync(
    path.join(rootDir, 'package.json'),
    `${JSON.stringify({ name: 'jx3-raid-manager', version: '2.1.18' }, null, 2)}\n`,
    'utf8'
  );
  fs.writeFileSync(
    path.join(rootDir, 'package-lock.json'),
    `${JSON.stringify(
      {
        name: 'jx3-raid-manager',
        version: '2.1.18',
        packages: {
          '': {
            version: '2.1.18',
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
    `${JSON.stringify({ version: '2.1.18' }, null, 2)}\n`,
    'utf8'
  );
  fs.writeFileSync(
    path.join(rootDir, 'src-tauri', 'Cargo.toml'),
    '[package]\nname = "jx3-raid-manager"\nversion = "2.1.18"\n',
    'utf8'
  );
  fs.writeFileSync(
    path.join(rootDir, 'src-tauri', 'Cargo.lock'),
    '[[package]]\nname = "jx3-raid-manager"\nversion = "2.1.18"\n',
    'utf8'
  );

  return rootDir;
}

afterEach(() => {
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
});
