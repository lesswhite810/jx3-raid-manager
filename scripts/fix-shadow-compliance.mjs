// R1 修复脚本:批量替换 shadow-md/lg → shadow-ds-stack;shadow-xl/2xl → shadow-ds-modal
// 按 design-tokens.md §3.1 映射
// shadow-md/lg  = 中等阴影(弹窗/浮层)   → shadow-ds-stack
// shadow-xl/2xl = 强阴影(Modal/Toast)    → shadow-ds-modal

import fs from 'node:fs';

const targets = ['components', 'src', 'App.tsx'];
const mapping = [
  // 顺序很重要:先匹配长的(xl/2xl)再匹配短的(md/lg),避免重叠
  [/\bshadow-2xl\b/g, 'shadow-ds-modal'],
  [/\bshadow-xl\b/g, 'shadow-ds-modal'],
  [/\bshadow-lg\b/g, 'shadow-ds-stack'],
  [/\bshadow-md\b/g, 'shadow-ds-stack'],
];

function walk(dir, list = []) {
  if (!fs.existsSync(dir)) return list;
  const s = fs.statSync(dir);
  if (s.isFile()) { list.push(dir); return list; }
  for (const e of fs.readdirSync(dir)) walk(dir + '/' + e, list);
  return list;
}
const files = targets.flatMap(t => walk(t)).filter(f => f.endsWith('.tsx') || f.endsWith('.ts'));

let totalReplacements = 0;
let fileCount = 0;
const details = [];

for (const f of files) {
  let text = fs.readFileSync(f, 'utf8');
  let fileReplacements = 0;
  for (const [re, replacement] of mapping) {
    text = text.replace(re, (match) => {
      fileReplacements++;
      return replacement;
    });
  }
  if (fileReplacements > 0) {
    fs.writeFileSync(f, text, 'utf8');
    fileCount++;
    totalReplacements += fileReplacements;
    details.push({ file: f.replace(/\\/g, '/'), count: fileReplacements });
  }
}

console.log('\n══════ R1 阴影规范合规修复 ══════\n');
for (const d of details) {
  console.log(`  ✓ ${d.count} 处  ${d.file}`);
}
console.log(`\n总计: ${fileCount} 个文件 · ${totalReplacements} 处替换`);
console.log('\n映射规则(specs/design-tokens.md §3.1):');
console.log('  shadow-md / shadow-lg   → shadow-ds-stack  (弹窗 / 浮层)');
console.log('  shadow-xl / shadow-2xl  → shadow-ds-modal  (Modal / Toast)');
console.log('\n注意:');
console.log('  • shadow-sm / shadow 保持不变(合规)');
console.log('  • shadow-ds-flat 是另一档(卡片悬浮),这次未触及');
console.log('  • 如需把 shadow-md 改为 shadow-ds-flat(更弱),手动逐处处理');
console.log('═══════════════════════════════════\n');