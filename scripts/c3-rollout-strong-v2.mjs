// C3.1 补充:C3 脚本漏掉的 case
//   ① text-[10px] / text-[11px] / text-[12px] 等自定义小字号(实际是小正文)
//   ② text-ds-{sem}/N  透明度修饰(透明度降低对比度)
//   ③ 图标 className 中(w-* h-* text-ds-{sem}) —— 图标小,需要更深的色
//   ④ Tag/Pill 上下文(bg-ds-{sem}-soft ... text-ds-{sem}) —— 软底上的文字色对比度不达 AA
//
// 策略:
//   - 同一行内出现"text-ds-{sem}" + "小正文标识"(text-[Npx] 或 text-xs 或 text-sm)
//                       + "透明度修饰或图标 size 或 Tag 软底" → 替换
//   - 静态 text-base/lg/xl/2xl/3xl/4xl 保留(大字号 AA 大字号合规)
//   - 没有上述任何标识 + 仅大字号 className 保留
//   - text-ds-{sem}/100 或单独图标大字号(w-6 h-6 以上)保留

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
  const EOL = s.includes('\r\n') ? '\r\n' : '\n';

  // 行级处理
  const lines = s.split(/\r?\n/);
  const newLines = [];

  for (const line of lines) {
    let newLine = line;

    // 检测"小正文上下文"
    const hasSmallNamed = /\btext-xs\b/.test(line) || /\btext-sm\b/.test(line);
    const hasSmallArbitrary = /\btext-\[\d+px\]/.test(line); // text-[10px] / text-[12px] 等
    const hasLargeText = /\btext-(base|lg|xl|2xl|3xl|4xl)\b/.test(line);

    // Tag/Pill 上下文(同 line 含 bg-ds-{sem}-soft)
    const hasTagSoft = semantics.some(sem => new RegExp(String.raw`\bbg-ds-` + sem + String.raw`-soft\b`).test(line));

    // 图标上下文(w-* h-* className 在同 line,且 text-ds-{sem} 是颜色)
    const hasIconSize = /\bw-\d+\b/.test(line) && /\bh-\d+\b/.test(line);
    // 透明度修饰(/{2,3} 数字)
    const hasOpacity = /\btext-ds-(success|warning|info)\/\d+/.test(line);

    // 替换条件:
    //   1) 小字号(text-xs/text-sm/text-[Npx]) 且 不含大字号 → 替换
    //   2) Tag/Pill 上下文(同 line) → 替换
    //   3) 透明度修饰 → 替换
    //   4) 小图标(text-ds-{sem} 用作图标色) → 替换
    const shouldReplace =
      (hasSmallNamed || hasSmallArbitrary) && !hasLargeText
      || hasTagSoft
      || hasOpacity
      || (hasIconSize && (hasSmallNamed || hasSmallArbitrary || hasOpacity || hasTagSoft));

    if (shouldReplace) {
      for (const sem of semantics) {
        // 替换 text-ds-{sem} (含 dark: 前缀、透明度修饰) → text-ds-{sem}-strong
        // 不替换已 strong 后缀的
        const before = newLine;
        newLine = newLine.replace(
          new RegExp(
            String.raw`\b(dark:)?(text-ds-` + sem + String.raw`)\b(?!-strong)(?!\/)`,
            'g'
          ),
          (m, dp) => (dp || '') + 'text-ds-' + sem + '-strong'
        );
        if (newLine !== before) fileReplacements++;

        // 处理 text-ds-{sem}/N 透明度修饰
        const before2 = newLine;
        newLine = newLine.replace(
          new RegExp(
            String.raw`\b(dark:)?(text-ds-` + sem + String.raw`)(?=\/\d+\b)`,
            'g'
          ),
          (m, dp) => (dp || '') + 'text-ds-' + sem + '-strong'
        );
        if (newLine !== before2) fileReplacements++;
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

console.log('\n═══ C3.1 补充替换报告 ═══\n');
for (const d of fileDetails) {
  console.log(`  ✓ ${d.count} 处  ${d.file}`);
}
console.log(`\n合计: ${fileDetails.length} 个文件 · ${totalReplacements} 处替换\n`);
console.log('触发条件:');
console.log('  ① text-xs / text-sm / text-[Npx] 自定义小字号');
console.log('  ② 同 line 含 bg-ds-{sem}-soft(Tag/Pill 上下文)');
console.log('  ③ text-ds-{sem}/N 透明度修饰');
console.log('  ④ 图标 + 上述任一标识');