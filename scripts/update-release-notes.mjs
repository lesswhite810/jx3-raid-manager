import { readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';

const [, , tag, notesFile] = process.argv;

if (!tag || !notesFile) {
  console.error('Usage: node scripts/update-release-notes.mjs <tag> <notes-file>');
  process.exit(1);
}

const requestJson = (args, input) => {
  const result = spawnSync('gh', args, {
    input,
    encoding: 'utf8',
    stdio: ['pipe', 'pipe', 'pipe']
  });

  if (result.status !== 0) {
    process.stderr.write(result.stderr);
    process.exit(result.status ?? 1);
  }

  return result.stdout;
};

const releaseRaw = requestJson([
  'api',
  'repos/lesswhite810/jx3-raid-manager/releases/tags/' + tag
]);
const release = JSON.parse(releaseRaw);
const body = readFileSync(notesFile, 'utf8');
const payload = JSON.stringify({ body });

requestJson([
  'api',
  'repos/lesswhite810/jx3-raid-manager/releases/' + release.id,
  '--method',
  'PATCH',
  '--input',
  '-'
], payload);

process.stdout.write(`Updated release notes for ${tag}\n`);
