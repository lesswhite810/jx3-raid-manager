// C3 铺开:智能批量替换所有"小正文"(text-xs / text-sm)里的
//   text-ds-success / text-ds-warning / text-ds-info
//   →  text-ds-{success,warning,info}-strong
//
// 策略:
//   - 同一行 className 同时含 text-{xs,sm} 且不含 text-{base,lg,xl,2xl,3xl,4xl} → 替换
//   - 仅含大字号 → 保留(已 AA 大字号合规)
//   - 没有 text-* size class 的 → 保留(默认值,通常是大字号或继承)
//
// 注意:仅替换 text-* 类名(bg-*-strong 不在本次范围)

import fs from 'node:fs';
import path from 'node:path';

function walk(dir, list = []) {
  if (!fs.existsSync(dir)) return list;
  const s = fs.statSync(dir);
  if (s.isFile()) { list.push(dir); return list; }
  for (const e of fs.readdirSync(dir)) walk(path.join(dir, e), list);
  return list;
}

const targets = ['components', 'src', 'App.tsx'];
const files = targets.flatMap(t => walk(t)).filter(f => f.endsWith('.tsx') || f.endsWith('.ts'));

const semantics = ['success', 'warning', 'info'];
let totalReplacements = 0;
const fileDetails = [];

for (const file of files) {
  const original = fs.readFileSync(file, 'utf8');
  let s = original;
  let fileReplacements = 0;

  for (const sem of semantics) {
    // 匹配 text-ds-{sem} 或 dark:text-ds-{sem} 但不在 strong 后
    // 用 negative lookbehind 避免替换已 strong 的
    const re = new RegExp(
      String.raw`(?<![\w-])(dark:)?text-ds-` + sem + String.raw`\b(?!-strong)`,
      'g'
    );

    s = s.replace(re, (match, darkPrefix) => {
      // 检查 match 所在行的上下文
      // 由于 JS replace 不直接给行号,用 lastIndex 估算前后行
      const matchIdx = s.lastIndexOf(match);  // 注意:这是 replace 前的 s,所以要重新算
      // 简化:对每个 match 检查整个文件原文中的行
      return match;  // placeholder
    });
  }

  // 上面那版太复杂,改成更简单但正确的实现:
  s = original;  // 重置
  fileReplacements = 0;

  const lines = s.split(/\r?\n/);
  const newLines = [];
  for (const line of lines) {
    let newLine = line;

    // 检查是否含 text-xs 或 text-sm,且不含大字号
    const hasSmallText = /\btext-xs\b/.test(line) || /\btext-sm\b/.test(line);
    const hasLargeText = /\btext-(base|lg|xl|2xl|3xl|4xl)\b/.test(line);

    if (hasSmallText && !hasLargeText) {
      for (const sem of semantics) {
        // 替换 text-ds-{sem}  → text-ds-{sem}-strong
        // 含 dark: 前缀同理
        // 仅在 text 类名后(已用 strong 后缀的跳过)
        const before = newLine;
        newLine = newLine.replace(
          new RegExp(String.raw`\b(dark:)?text-ds-` + sem + String.raw`\b(?!-strong)`, 'g'),
          (m, dp) => (dp || '') + 'text-ds-' + sem + '-strong'
        );
        if (newLine !== before) fileReplacements++;
      }
    }

    newLines.push(newLine);
  }

  if (fileReplacements > 0) {
    s = newLines.join(/\r?\n/.test(s) ? '\r\n' : '\n');
    fs.writeFileSync(file, s, 'utf8');
    fileDetails.push({ file: file.replace(/\\/g, '/'), count: fileReplacements });
    totalReplacements += fileReplacements;
  }
}

console.log('\n═══ C3 铺开 · 批量替换报告 ═══\n');
for (const d of fileDetails) {
  console.log(`  ✓ ${d.count} 处  ${d.file}`);
}
console.log(`\n合计: ${fileDetails.length} 个文件 · ${totalReplacements} 处替换\n`);
console.log('策略:');
console.log('  • text-xs / text-sm + text-ds-{success,warning,info} → 替换为 *-strong');
console.log('  • text-{base,lg,xl,2xl,3xl,4xl} → 保留(已 AA 大字号合规)');
console.log('  • 没有 text-* size class 的 → 保留(默认值通常是大字号)');
console.log('  • 不动 bg-* 类名(背景色已有 *-soft)\n');