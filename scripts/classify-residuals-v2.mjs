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
const noSizeButFontBold = [];

for (const file of files) {
  const text = fs.readFileSync(file, 'utf8');
  const lines = text.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    if (!re.test(lines[i])) continue;
    re.lastIndex = 0;
    const line = lines[i];
    const hasText = /\btext-(?:xs|sm|base|lg|xl|2xl|3xl|4xl)\b/.test(line);
    const hasTag = /\bbg-ds-(?:success|warning|info)-soft\b/.test(line);
    const hasOpacity = /\btext-ds-(?:success|warning|info)\/\d+/.test(line);
    const hasIcon = /\bw-\d+\b/.test(line) && /\bh-\d+\b/.test(line);
    const hasArbitrary = /\btext-\[\d+px\]/.test(line);
    if (hasText || hasTag || hasOpacity || hasIcon || hasArbitrary) continue;
    if (!hasText) {
      const hasBold = /\bfont-bold\b/.test(line) || /\bfont-semibold\b/.test(line);
      const has1Rem = /\btext-\[1rem\]/.test(line);
      noSizeButFontBold.push({
        file: file.replace(/\\/g, '/'),
        line: i + 1,
        content: line.trim().substring(0, 110),
        bold: hasBold,
        rem: has1Rem,
      });
    }
  }
}

console.log('「没有 text-* 类」剩余位置的子分类:');
const withBold = noSizeButFontBold.filter(s => s.bold).length;
const with1Rem = noSizeButFontBold.filter(s => s.rem).length;
const pureIcon = noSizeButFontBold.filter(s => s.content.includes('w-') || s.content.includes('h-')).length;
const pureOther = noSizeButFontBold.length - withBold - with1Rem;
console.log('  含 font-bold / font-semibold:', withBold);
console.log('  含 text-[1rem]:', with1Rem);
console.log('  纯图标 (w-*/h-*):', pureIcon);
console.log('  其它:', pureOther);
console.log('');
console.log('▌ 字体加粗样本(font-bold + text-ds-success 默认 16px):');
for (const s of noSizeButFontBold.filter(s => s.bold).slice(0, 6)) {
  console.log('  ' + s.file + ':' + s.line);
  console.log('    ' + s.content);
}
console.log('');
console.log('▌ text-[1rem] 样本:');
for (const s of noSizeButFontBold.filter(s => s.rem).slice(0, 3)) {
  console.log('  ' + s.file + ':' + s.line);
  console.log('    ' + s.content);
}