// C3.2 收口:图标颜色 + 对象属性 className 字面值 + truncate 文本
//   - 图标 className (Lucide React / 自定义 Icon) 中的 text-ds-{sem} → strong
//   - 对象属性里的 className 字面值 (iconColor: 'text-ds-warning') → strong
//   - truncate 文本中的 text-ds-{sem} → strong
//
// 策略:同 line 同时含"小标识上下文"(小图标尺寸 / 属性赋值 / truncate)才替换
//   保留:大字号 / 大图标(w-5+ / h-5+) / font-bold 默认 / text-[1rem] 等合规场景

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

const semantics = ['success', 'warning', 'info'];
let totalReplacements = 0;
const fileDetails = [];

for (const file of files) {
  const original = fs.readFileSync(file, 'utf8');
  let s = original;
  let fileReplacements = 0;

  const lines = s.split(/\r?\n/);
  const newLines = [];

  for (const line of lines) {
    let newLine = line;
    let touched = false;

    // 保留条件:已是大字号 / 大图标 / 字体加粗
    const hasLargeText = /\btext-(base|lg|xl|2xl|3xl|4xl)\b/.test(line);
    const hasFontBold = /\bfont-bold\b/.test(line);
    const hasLargeIcon = /\bw-(?:[5-9]|\d{2,})\b/.test(line) || /\bh-(?:[5-9]|\d{2,})\b/.test(line);

    if (hasLargeText || hasFontBold || hasLargeIcon) {
      newLines.push(line);
      continue;
    }

    // 替换条件:图标 / 属性赋值 / truncate / text-[1rem] + 同 line 有 ds-{sem}
    const hasSmallIcon = /\bw-[1-4](?:\.\d+)?\b/.test(line) && /\bh-[1-4](?:\.\d+)?\b/.test(line);
    const hasPropAssign = /(?:iconColor|color|className|class)\s*:\s*['"`]/.test(line);
    const hasTruncate = /\btruncate\b/.test(line);
    const has1Rem = /\btext-\[1rem\]/.test(line);

    const shouldReplace = (hasSmallIcon || hasPropAssign || hasTruncate || has1Rem)
      && semantics.some(sem => new RegExp(String.raw`\btext-ds-` + sem + String.raw`\b(?!-strong)`).test(line));

    if (shouldReplace) {
      for (const sem of semantics) {
        const before = newLine;
        newLine = newLine.replace(
          new RegExp(String.raw`\btext-ds-` + sem + String.raw`\b(?!-strong)`, 'g'),
          'text-ds-' + sem + '-strong'
        );
        if (newLine !== before) {
          fileReplacements++;
          touched = true;
        }
      }
    }

    newLines.push(newLine);
  }

  if (fileReplacements > 0) {
    s = newLines.join(/\r?\n/.test(original) ? '\r\n' : '\n');
    fs.writeFileSync(file, s, 'utf8');
    fileDetails.push({ file: file.replace(/\\/g, '/'), count: fileReplacements });
    totalReplacements += fileReplacements;
  }
}

console.log('\n═══ C3.2 收口替换报告 ═══\n');
for (const d of fileDetails) {
  console.log(`  ✓ ${d.count} 处  ${d.file}`);
}
console.log(`\n合计: ${fileDetails.length} 个文件 · ${totalReplacements} 处替换\n`);
console.log('触发条件:');
console.log('  • 小图标 className (w-[1-4] h-[1-4])');
console.log('  • 对象属性 className 字面值 (iconColor / color / className / class)');
console.log('  • truncate 文本');
console.log('  • text-[1rem] 自定义字号(16px)');
console.log('');
console.log('保留条件:');
console.log('  • 大字号 text-base/lg/xl/2xl/3xl/4xl');
console.log('  • 字体加粗 font-bold');
console.log('  • 大图标 w-[5-9] / h-[5-9]');