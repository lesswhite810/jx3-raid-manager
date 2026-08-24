// 验证 C3 铺开后,原 text-ds-success/warning/info 残留是否符合"应保留"规则
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

const re = /(?<![\w-])text-ds-(success|warning|info)\b(?!-strong)/g;

let largeKept = 0;
let noSizeKept = 0;
let smallShouldReplace = 0;
const smallHits = [];

for (const file of files) {
  const text = fs.readFileSync(file, 'utf8');
  const lines = text.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    if (!re.test(lines[i])) continue;
    re.lastIndex = 0;
    const line = lines[i];
    const hasSmall = /\btext-xs\b/.test(line) || /\btext-sm\b/.test(line);
    const hasLarge = /\btext-(base|lg|xl|2xl|3xl|4xl)\b/.test(line);
    const hasText = /\btext-(?:xs|sm|base|lg|xl|2xl|3xl|4xl)\b/.test(line);

    if (hasSmall && !hasLarge) {
      smallShouldReplace++;
      smallHits.push({ file: file.replace(/\\/g, '/'), line: i + 1, content: line.trim().substring(0, 80) });
    } else if (hasLarge) {
      largeKept++;
    } else if (!hasText) {
      noSizeKept++;
    }
  }
}

console.log('残留分类:');
console.log('  大字号(text-base/lg/xl/2xl/3xl):', largeKept);
console.log('  没有 text-* 字号类:', noSizeKept);
console.log('  小正文未替换(应=0):', smallShouldReplace);
if (smallShouldReplace > 0) {
  console.log('');
  console.log('═══ 应替换但未替换的位置 ═══');
  for (const h of smallHits.slice(0, 20)) {
    console.log('  ' + h.file + ':' + h.line);
    console.log('    ' + h.content);
  }
}