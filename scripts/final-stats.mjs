import fs from 'node:fs';
import path from 'node:path';

function walk(dir, list = []) {
  if (!fs.existsSync(dir)) return list;
  const s = fs.statSync(dir);
  if (s.isFile()) { list.push(dir); return list; }
  for (const e of fs.readdirSync(dir)) walk(path.join(dir, e), list);
  return list;
}
const files = ['components', 'src', 'App.tsx'].flatMap(t => walk(t)).filter(f => f.endsWith('.tsx') || f.endsWith('.ts'));

const seenLines = new Set();
let strongLines = 0;
let plainLines = 0;
const plainFiles = new Map();

for (const file of files) {
  const text = fs.readFileSync(file, 'utf8');
  const lines = text.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const hasStrong = /\btext-ds-(?:success|warning|info)-strong\b/.test(line);
    const hasPlain = /\btext-ds-(?:success|warning|info)\b(?!\-strong)/.test(line);

    if (hasStrong && !seenLines.has(file + ':' + i + ':strong')) {
      seenLines.add(file + ':' + i + ':strong');
      strongLines++;
    }
    if (hasPlain && !seenLines.has(file + ':' + i + ':plain')) {
      seenLines.add(file + ':' + i + ':plain');
      plainLines++;
      const f = file.replace(/\\/g, '/');
      plainFiles.set(f, (plainFiles.get(f) || 0) + 1);
    }
  }
}

console.log('按「文件-行号」去重统计:');
console.log('  含 *-strong 的行:', strongLines);
console.log('  含原色的行:', plainLines);
console.log('');
console.log('原色行分布(按文件,前 15):');
const sorted = [...plainFiles.entries()].sort((a,b) => b[1] - a[1]).slice(0, 15);
for (const [f, n] of sorted) {
  console.log('  ' + n + '  ' + f);
}