// 分类剩余 "没有 text-* 字号类" 的位置,看是否还需要替换
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
const samples = [];

for (const file of files) {
  const text = fs.readFileSync(file, 'utf8');
  const lines = text.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    if (!re.test(lines[i])) continue;
    re.lastIndex = 0;
    const line = lines[i];
    const hasText = /\btext-(?:xs|sm|base|lg|xl|2xl|3xl|4xl)\b/.test(line);
    if (!hasText) {
      samples.push({
        file: file.replace(/\\/g, '/'),
        line: i + 1,
        content: line.trim().substring(0, 100),
        tag: /\bbg-ds-(?:success|warning|info)-soft\b/.test(line),
        opacity: /\btext-ds-(?:success|warning|info)\/\d+/.test(line),
        icon: /\bw-\d+\b/.test(line) && /\bh-\d+\b/.test(line),
        arbitrary: /\btext-\[\d+px\]/.test(line),
      });
    }
  }
}

const byReason = {};
for (const s of samples) {
  let reason = '其它';
  if (s.tag) reason = 'Tag/Pill 上下文';
  else if (s.opacity) reason = '透明度修饰';
  else if (s.icon) reason = '图标颜色';
  else if (s.arbitrary) reason = 'text-[Npx] 自定义字号';
  else if (s.content.includes('truncate')) reason = 'truncate 文本';
  else if (/^[^<>]*=/.test(s.content)) reason = '模板字符串';
  byReason[reason] = byReason[reason] || [];
  byReason[reason].push(s);
}

console.log('「没有 text-* 字号类」剩余位置:', samples.length);
console.log('');
for (const [k, v] of Object.entries(byReason)) {
  console.log('  ' + k + ': ' + v.length + ' 处');
}

console.log('\n═══ 各原因样本 ═══');
for (const [reason, ss] of Object.entries(byReason)) {
  console.log('\n▌ ' + reason + ' (前 5 条)');
  for (const s of ss.slice(0, 5)) {
    console.log('  ' + s.file + ':' + s.line);
    console.log('    ' + s.content);
  }
}