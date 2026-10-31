import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { afterEach, expect, test } from 'vitest';
import { syncGiteeMirror } from './sync-gitee-mirror.mjs';

const fixtureRoots = [];
const removeFixtureRoot = (root) => {
  const tempRoot = resolve(tmpdir());
  const resolvedRoot = resolve(root);
  if (dirname(resolvedRoot) !== tempRoot || !basename(resolvedRoot).startsWith('jx3-gitee-mirror-')) {
    throw new Error('Refusing to remove a non-fixture directory');
  }
  rmSync(resolvedRoot, { recursive: true, force: true });
};
afterEach(() => {
  for (const root of fixtureRoots.splice(0)) {
    removeFixtureRoot(root);
  }
});

const git = (cwd, args) => execFileSync('git', args, {
  cwd,
  encoding: 'utf8',
  stdio: ['ignore', 'pipe', 'pipe'],
}).trim();
const bareGit = (repository, args) => execFileSync('git', ['--git-dir', repository, ...args], {
  encoding: 'utf8',
  stdio: ['ignore', 'pipe', 'pipe'],
}).trim();

const configureUser = (cwd) => {
  git(cwd, ['config', 'user.name', 'Mirror Test']);
  git(cwd, ['config', 'user.email', 'mirror-test@example.invalid']);
};

const createFixture = () => {
  const root = mkdtempSync(join(tmpdir(), 'jx3-gitee-mirror-'));
  fixtureRoots.push(root);
  const origin = join(root, 'origin.git');
  const target = join(root, 'target.git');
  const source = join(root, 'source');
  const runner = join(root, 'runner');
  const targetWriter = join(root, 'target-writer');
  git(root, ['init', '--bare', '--initial-branch=master', origin]);
  git(root, ['init', '--bare', '--initial-branch=master', target]);

  git(root, ['clone', origin, source]);
  configureUser(source);
  writeFileSync(join(source, 'tracked.txt'), 'first\n');
  git(source, ['add', 'tracked.txt']);
  git(source, ['commit', '-m', 'initial']);
  git(source, ['tag', '-a', 'v1.0.0', '-m', 'annotated source tag']);
  git(source, ['tag', 'v1.0.1']);
  git(source, ['push', 'origin', 'master', '--tags']);

  // Both the eventual mirror runner and target begin behind the later source update.
  git(root, ['clone', origin, runner]);
  git(root, ['clone', origin, targetWriter]);
  configureUser(targetWriter);
  git(targetWriter, ['remote', 'add', 'target', target]);
  git(targetWriter, ['push', 'target', 'master']);
  // Same commit but a different annotated-tag object: comparing only peeled commits
  // would miss this mirror divergence.
  git(targetWriter, ['tag', '-f', '-a', 'v1.0.0', '-m', 'different target annotation']);
  git(targetWriter, ['push', '--force', 'target', 'v1.0.0']);
  git(targetWriter, ['checkout', '-b', 'updater-assets']);
  writeFileSync(join(targetWriter, 'asset.txt'), 'preserve this branch\n');
  git(targetWriter, ['add', 'asset.txt']);
  git(targetWriter, ['commit', '-m', 'updater asset']);
  git(targetWriter, ['push', 'target', 'updater-assets']);
  git(targetWriter, ['checkout', 'master']);
  git(targetWriter, ['tag', '-a', 'v1.2.1', '-m', 'target-only annotated tag']);
  git(targetWriter, ['push', 'target', 'v1.2.1']);

  writeFileSync(join(source, 'tracked.txt'), 'second\n');
  git(source, ['commit', '-am', 'advance source master']);
  git(source, ['tag', '-a', 'v1.1.0', '-m', 'second annotated source tag']);
  git(source, ['push', 'origin', 'master', '--tags']);

  return { root, origin, target, source, runner };
};

const createMinimalRunner = () => {
  const root = mkdtempSync(join(tmpdir(), 'jx3-gitee-mirror-'));
  fixtureRoots.push(root);
  const origin = join(root, 'origin.git');
  const runner = join(root, 'runner');
  git(root, ['init', '--bare', '--initial-branch=master', origin]);
  git(root, ['clone', origin, runner]);
  configureUser(runner);
  writeFileSync(join(runner, 'tracked.txt'), 'source\n');
  git(runner, ['add', 'tracked.txt']);
  git(runner, ['commit', '-m', 'source']);
  git(runner, ['push', 'origin', 'master']);
  return { root, runner };
};

const tagObjectAndPeeled = (repository, tag) => ({
  object: bareGit(repository, ['rev-parse', `refs/tags/${tag}`]),
  peeled: bareGit(repository, ['rev-parse', `refs/tags/${tag}^{}`]),
});

test('refreshes stale origin tracking, mirrors master and tags, and preserves extras', async () => {
  const fixture = createFixture();
  const sourceAnnotation = tagObjectAndPeeled(fixture.origin, 'v1.0.0');
  const targetAnnotationBefore = tagObjectAndPeeled(fixture.target, 'v1.0.0');
  expect(targetAnnotationBefore.peeled).toBe(sourceAnnotation.peeled);
  expect(targetAnnotationBefore.object).not.toBe(sourceAnnotation.object);

  const summary = await syncGiteeMirror({ cwd: fixture.runner, targetUrl: fixture.target });
  expect(summary.status).toBe('synced');
  expect(summary.extraTags).toEqual(['v1.2.1']);
  expect(bareGit(fixture.target, ['rev-parse', 'refs/heads/master']))
    .toBe(bareGit(fixture.origin, ['rev-parse', 'refs/heads/master']));
  for (const tag of ['v1.0.0', 'v1.0.1', 'v1.1.0']) {
    expect(tagObjectAndPeeled(fixture.target, tag)).toEqual(tagObjectAndPeeled(fixture.origin, tag));
  }
  expect(tagObjectAndPeeled(fixture.target, 'v1.2.1').peeled)
    .toBe(bareGit(fixture.target, ['rev-parse', 'refs/heads/master^']));
  expect(bareGit(fixture.target, ['show-ref', '--verify', 'refs/heads/updater-assets']))
    .toMatch(/^[0-9a-f]{40}/);

  const repeated = await syncGiteeMirror({ cwd: fixture.runner, targetUrl: fixture.target });
  expect(repeated.status).toBe('synced');
  expect(repeated.extraTags).toEqual(['v1.2.1']);
});

test('prunes only explicit extra tags while preserving updater-assets', async () => {
  const fixture = createFixture();
  const extraBeforePrune = tagObjectAndPeeled(fixture.target, 'v1.2.1');
  expect(extraBeforePrune.object).not.toBe(extraBeforePrune.peeled);
  const script = readFileSync(new URL('./sync-gitee-mirror.mjs', import.meta.url), 'utf8');
  expect(script).toContain('`--force-with-lease=refs/tags/${name}:${targetTag.object}`');

  const summary = await syncGiteeMirror({
    cwd: fixture.runner,
    targetUrl: fixture.target,
    pruneTags: true,
  });
  expect(summary.prunedTags).toEqual(['v1.2.1']);
  expect(bareGit(fixture.target, ['show-ref', '--tags'])).not.toMatch(/refs\/tags\/v1\.2\.1$/m);
  expect(bareGit(fixture.target, ['show-ref', '--verify', 'refs/heads/updater-assets']))
    .toMatch(/^[0-9a-f]{40}/);
});

test('missing target may only be skipped when explicitly optional', async () => {
  const fixture = createMinimalRunner();
  await expect(syncGiteeMirror({ cwd: fixture.runner, targetUrl: '' }))
    .rejects.toThrow(/GITEE_PUSH_URL is required/);
  await expect(
    syncGiteeMirror({ cwd: fixture.runner, targetUrl: join(fixture.root, 'missing.git') }),
  ).rejects.toThrow(/Read mirror refs failed/);
  await expect(syncGiteeMirror({ cwd: fixture.runner, targetUrl: '', skipIfMissingTarget: true }))
    .resolves.toEqual({ status: 'skipped', reason: 'missing_gitee_push_url' });

  const secretUrl = 'https://user:very-secret@127.0.0.1:1/repo.git';
  const error = await syncGiteeMirror({ cwd: fixture.runner, targetUrl: secretUrl }).catch(reason => reason);
  expect(error.message).toMatch(/Read mirror refs failed/);
  expect(error.message).not.toContain('very-secret');
  expect(error.message).not.toContain(secretUrl);
});

test('release and manual workflows use the shared secret-bound mirror script after version preparation', () => {
  const release = readFileSync(new URL('../.github/workflows/release.yml', import.meta.url), 'utf8');
  const manual = readFileSync(new URL('../.github/workflows/gitee-mirror.yml', import.meta.url), 'utf8');
  const mirrorJobStart = release.indexOf('\n  mirror-code:\n');
  const mirrorJobEnd = release.indexOf('\n  success:', mirrorJobStart);
  expect(mirrorJobStart).toBeGreaterThanOrEqual(0);
  expect(mirrorJobEnd).toBeGreaterThan(mirrorJobStart);
  const mirrorJob = release.slice(mirrorJobStart, mirrorJobEnd);

  expect(mirrorJob).toMatch(/needs: release/);
  expect(mirrorJob).toMatch(/group: gitee-code-mirror/);
  expect(mirrorJob).toMatch(/node scripts\/sync-gitee-mirror\.mjs --skip-if-missing-target/);
  expect(mirrorJob).toMatch(/GITEE_PUSH_URL:\s*\$\{\{ secrets\.GITEE_PUSH_URL \}\}/);

  expect(manual).toMatch(/workflow_dispatch:/);
  expect(manual).toMatch(/prune_tags:/);
  expect(manual).toMatch(/contents: read/);
  expect(manual).toMatch(/group: gitee-code-mirror/);
  expect(manual).toMatch(/sync-gitee-mirror\.mjs "\$\{args\[@\]\}"/);
  expect(manual).toMatch(/GITEE_PUSH_URL:\s*\$\{\{ secrets\.GITEE_PUSH_URL \}\}/);
});
