import { spawnSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';

const RETRY_ATTEMPTS = 3;

class MirrorError extends Error {}

const sleep = (milliseconds) => new Promise(resolve => setTimeout(resolve, milliseconds));

const runGit = (args, { cwd, label }) => {
  const result = spawnSync('git', args, {
    cwd,
    encoding: 'utf8',
    windowsHide: true,
  });
  if (result.status !== 0) {
    // Git can echo an authenticated remote URL in stderr. Keep CLI errors deliberately
    // generic so Actions logs never expose a credential embedded in GITEE_PUSH_URL.
    throw new MirrorError(`${label} failed (exit ${result.status ?? 'unknown'})`);
  }
  return result.stdout;
};

const runGitWithRetry = async (args, options) => {
  let lastError;
  for (let attempt = 1; attempt <= RETRY_ATTEMPTS; attempt += 1) {
    try {
      return runGit(args, options);
    } catch (error) {
      lastError = error;
      if (attempt < RETRY_ATTEMPTS) await sleep(250 * attempt);
    }
  }
  throw lastError;
};

const parseRemoteRefs = (output) => {
  const master = { object: null };
  const tags = new Map();

  for (const line of output.split(/\r?\n/)) {
    if (!line) continue;
    const [object, ref] = line.split('\t');
    if (ref === 'refs/heads/master') {
      master.object = object;
      continue;
    }
    if (!ref?.startsWith('refs/tags/')) continue;

    const peeled = ref.endsWith('^{}');
    const name = ref.slice('refs/tags/'.length, peeled ? -3 : undefined);
    const tag = tags.get(name) ?? { object: null, peeled: null };
    if (peeled) tag.peeled = object;
    else tag.object = object;
    tags.set(name, tag);
  }

  return { master, tags };
};

const readRemoteRefs = (remote, { cwd, label }) => parseRemoteRefs(
  runGit(['ls-remote', '--heads', '--tags', remote], { cwd, label })
);

const isSameTag = (source, target) => source?.object === target?.object
  && source?.peeled === target?.peeled;

const verifyMirror = ({ source, target, requireNoExtraTags }) => {
  const masterMatches = source.master.object !== null && source.master.object === target.master.object;
  const tagMismatches = [];
  for (const [name, sourceTag] of source.tags) {
    if (!isSameTag(sourceTag, target.tags.get(name))) tagMismatches.push(name);
  }
  const extraTags = [...target.tags.keys()].filter(name => !source.tags.has(name)).sort();
  if (!masterMatches || tagMismatches.length > 0 || (requireNoExtraTags && extraTags.length > 0)) {
    throw new MirrorError('Mirror verification failed: source refs differ from target refs');
  }
  return {
    master: source.master.object,
    tagCount: source.tags.size,
    extraTags,
  };
};

const pushMaster = async ({ cwd, targetUrl, sourceMaster, targetMaster }) => {
  const args = ['push', `--force-with-lease=refs/heads/master:${targetMaster ?? ''}`];
  args.push(targetUrl, `${sourceMaster}:refs/heads/master`);
  await runGitWithRetry(args, { cwd, label: 'Push master to mirror' });
};

const pushSourceTags = async ({ cwd, targetUrl, sourceTags, targetTags }) => {
  const changedTags = [...sourceTags]
    .filter(([name, sourceTag]) => !isSameTag(sourceTag, targetTags.get(name)));
  if (changedTags.length === 0) return;

  // One push avoids 65 HTTPS round-trips. Every ref still carries its own lease;
  // final remote verification rejects any partial result or concurrent divergence.
  const args = ['push'];
  for (const [name] of changedTags) {
    args.push(`--force-with-lease=refs/tags/${name}:${targetTags.get(name)?.object ?? ''}`);
  }
  args.push(targetUrl, ...changedTags.map(([name]) => `refs/tags/${name}:refs/tags/${name}`));
  await runGitWithRetry(args, { cwd, label: 'Push source tags to mirror' });
};

const pruneExtraTag = async ({ cwd, targetUrl, name, targetTag }) => {
  const args = [
    'push',
    `--force-with-lease=refs/tags/${name}:${targetTag.object}`,
    targetUrl,
    `:refs/tags/${name}`,
  ];
  await runGitWithRetry(args, { cwd, label: `Prune tag ${name} from mirror` });
};

/**
 * Synchronize only GitHub's master and tags to a Gitee source mirror.
 * Other target branches (notably updater-assets) are never touched.
 */
export const syncGiteeMirror = async ({
  cwd = process.cwd(),
  targetUrl = process.env.GITEE_PUSH_URL,
  pruneTags = false,
  skipIfMissingTarget = false,
} = {}) => {
  if (!targetUrl) {
    if (skipIfMissingTarget) {
      return { status: 'skipped', reason: 'missing_gitee_push_url' };
    }
    throw new MirrorError('GITEE_PUSH_URL is required for a manual mirror run');
  }

  // Refresh the GitHub source first. Do not rely on the checkout's possibly stale
  // origin/master or tag set when a release has just advanced master.
  await runGitWithRetry(
    ['fetch', '--force', 'origin', '+refs/heads/master:refs/remotes/origin/master', '+refs/tags/*:refs/tags/*'],
    { cwd, label: 'Refresh GitHub master and tags' },
  );

  const source = readRemoteRefs('origin', { cwd, label: 'Read GitHub refs' });
  if (!source.master.object) {
    throw new MirrorError('GitHub source does not expose refs/heads/master');
  }
  const targetBefore = readRemoteRefs(targetUrl, { cwd, label: 'Read mirror refs' });

  if (source.master.object !== targetBefore.master.object) {
    await pushMaster({
      cwd,
      targetUrl,
      sourceMaster: source.master.object,
      targetMaster: targetBefore.master.object,
    });
  }
  await pushSourceTags({ cwd, targetUrl, sourceTags: source.tags, targetTags: targetBefore.tags });

  const extraTagsBeforePrune = [...targetBefore.tags.keys()]
    .filter(name => !source.tags.has(name))
    .sort();
  if (pruneTags) {
    for (const name of extraTagsBeforePrune) {
      await pruneExtraTag({ cwd, targetUrl, name, targetTag: targetBefore.tags.get(name) });
    }
  }

  // GitHub may advance while this job is running. Re-read both remotes so a run never
  // reports success for the initial snapshot when the source has already changed.
  const sourceAfter = readRemoteRefs('origin', { cwd, label: 'Verify GitHub refs' });
  const targetAfter = readRemoteRefs(targetUrl, { cwd, label: 'Verify mirror refs' });
  const verification = verifyMirror({
    source: sourceAfter,
    target: targetAfter,
    requireNoExtraTags: pruneTags,
  });
  return {
    status: 'synced',
    sourceMaster: sourceAfter.master.object,
    sourceTagCount: sourceAfter.tags.size,
    extraTags: verification.extraTags,
    prunedTags: pruneTags ? extraTagsBeforePrune : [],
    verification: {
      masterMatches: true,
      tagsMatch: true,
      tagCount: verification.tagCount,
    },
  };
};

const parseCliOptions = (args) => ({
  pruneTags: args.includes('--prune-tags'),
  skipIfMissingTarget: args.includes('--skip-if-missing-target'),
});

const isCli = process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url;
if (isCli) {
  try {
    const summary = await syncGiteeMirror(parseCliOptions(process.argv.slice(2)));
    process.stdout.write(`${JSON.stringify(summary)}\n`);
  } catch (error) {
    const message = error instanceof Error ? error.message : 'Mirror failed';
    process.stderr.write(`Gitee source mirror failed: ${message}\n`);
    process.exitCode = 1;
  }
}
