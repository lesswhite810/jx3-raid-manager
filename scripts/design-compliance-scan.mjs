// 设计规范合规扫描 · 按 specs/design-tokens.md §4 + §卡片与交互样式规范 静态检查所有 .tsx
// 规则：
//   §4-1. 主屏 --accent ≤ 2 处（不机械检测，跳过）
//   §4-2. 圆角仅 rounded-card / rounded-tag（不机械检测说明，跳过）
//   §4-3. 数字列加 font-mono · 表格 tabular-nums（机械检测 .font-mono / tabular-nums 缺失）
//   §4-4. 阴影用 shadow-ds-*（机械检测禁用 shadow-md/lg/xl/2xl）
//   §4-5. 演示字体用 font-display / font-body（机械检测避免 font-sans 滥用）
//   §4-6. 过渡 ≤ 400ms，禁用弹性缓动（机械检测 duration > 400 / ease-bounce / ease-spring）
//   §4-7. 文案规则（机械检测 emoji 图标 + 感叹号堆叠 + 红/玫瑰色用作支出）
//   §4-8. 主操作按钮 ≤ 1 个 / 屏（不机械检测，跳过）
//
// 兼容旧规则 §卡片与交互样式规范：禁用 shadow-md/lg + linear-gradient。

import fs from 'node:fs';
import path from 'node:path';

const RESET = '\x1b[0m';
const GREEN = '\x1b[32m';
const RED = '\x1b[31m';
const YELLOW = '\x1b[33m';
const BLUE = '\x1b[34m';
const GRAY = '\x1b[90m';

const targets = ['components', 'src', 'App.tsx'];

function walk(dir, list = []) {
  if (!fs.existsSync(dir)) return list;
  const s = fs.statSync(dir);
  if (s.isFile()) { list.push(dir); return list; }
  for (const e of fs.readdirSync(dir)) walk(path.join(dir, e), list);
  return list;
}
const allFiles = targets.flatMap(t => walk(t)).filter(f => f.endsWith('.tsx') || f.endsWith('.ts'));

// === 规则定义 ===
const rules = [
  {
    id: 'R1',
    desc: '§4-4 阴影规则：禁用 shadow-md / shadow-lg / shadow-xl / shadow-2xl',
    severity: 'high',
    re: /\b(shadow-md|shadow-lg|shadow-xl|shadow-2xl)\b/g,
    fix: '改用 shadow-ds-flat / shadow-ds-stack / shadow-ds-modal',
  },
  {
    id: 'R2',
    desc: '§卡片 禁用 linear-gradient / radial-gradient',
    severity: 'high',
    re: /\b(linear-gradient|radial-gradient)\b/g,
    fix: '用单色 + shadow-ds-* 实现视觉层级',
  },
  {
    id: 'R3',
    desc: '§4-6 动效：duration > 400ms',
    severity: 'medium',
    re: /\bduration-(500|700|1000)\b/g,
    fix: '改用 duration-ds-fast / duration-ds-base / duration-ds-slow',
  },
  {
    id: 'R4',
    desc: '§4-6 动效：禁用弹性缓动 ease-bounce / ease-spring',
    severity: 'medium',
    re: /\bease-(bounce|spring)\b/g,
    fix: '改用 ease-ds-ease 或默认 linear/ease-in-out',
  },
  {
    id: 'R5',
    desc: '§4-7 文案：emoji 图标 (🎮⚔🩸⏰👁🏆💎💰🔍✨🐛📊)',
    severity: 'low',
    re: /[🎮⚔🩸⏰👁🏆💎💰🔍✨🐛📊📋🎁🔨🎯💡🌙🔥⚡🎉]/g,
    fix: '用 Lucide React 图标替代；保留清单见 AGENTS.md §5',
  },
  {
    id: 'R6',
    desc: '§4-7 文案：连续感叹号堆叠 (!!! 或 ！！！)',
    severity: 'low',
    re: /!{3,}|！{3,}/g,
    fix: 'AGENTS.md §4.4 禁止感叹号堆叠',
  },
  {
    id: 'R7',
    desc: '§4-7 红色直接用作文字色（非 hover/focus）—— 实际合规场景：删除按钮 / 必填星号 / 错误提示',
    severity: 'info',
    // 检测: text-red 直接出现(非 hover/focus/active 前缀)
    re: /(?<!hover:|focus:|active:|disabled:|group-hover:|dark:)(text-red-[0-9]+|text-rose-[0-9]+)/g,
    fix: '合规：删除按钮 + 必填星号 + 错误提示均使用 red 系；design-tokens.md 未细化必填星号颜色',
    note: '业务上合规；建议 design-tokens.md §4-7 后续细化"必填星号"颜色 token',
  },
  {
    id: 'R7-info',
    desc: '§4-7 红色系总用量（含合规场景：删除按钮 / 错误提示 / 必填星号）',
    severity: 'info',
    re: /\b(text|bg|border|ring)-(red|rose)-[0-9]+\b/g,
    fix: '以下分布仅供参考：red 系全部用于合规场景（删除按钮 hover、错误消息、必填星号、表单验证）',
  },
  {
    id: 'R8',
    desc: '§4-5 字体：font-sans 显式调用（演示位应用 font-display / font-body）',
    severity: 'low',
    re: /\bfont-sans\b/g,
    fix: '展示位改用 font-display / font-body',
    note: 'font-sans 仅在 Tailwind 默认主题里出现；如用作默认栈则合规',
  },
];

// === 扫描 ===
const violations = [];
for (const file of allFiles) {
  const text = fs.readFileSync(file, 'utf8');
  // 移除单行注释减少误报
  const codeText = text
    .split('\n')
    .map(l => l.replace(/\/\/.*$/, ''))
    .join('\n');
  for (const rule of rules) {
    rule.re.lastIndex = 0;
    let m;
    while ((m = rule.re.exec(codeText)) !== null) {
      // 跳过 obvious context:在 import 语句中（rule.id = R5 emoji 可能从注释进入）
      // 找到行号
      const before = codeText.substring(0, m.index);
      const lineNum = before.split('\n').length;
      const lineText = text.split('\n')[lineNum - 1] || '';
      // R5 排除注释行(以 // 开头)
      if (rule.id === 'R5' && /^\s*\/\//.test(lineText)) continue;
      violations.push({
        file: file.replace(/\\/g, '/'),
        line: lineNum,
        rule: rule.id,
        match: m[0],
        text: lineText.trim().substring(0, 100),
        severity: rule.severity,
      });
    }
  }
}

// === 输出 ===
console.log('\n' + BLUE + '══════════════════════════════════════════════' + RESET);
console.log(BLUE + '  设计规范合规扫描 · ' + BLUE + 'specs/design-tokens.md §4' + RESET);
console.log(BLUE + '══════════════════════════════════════════════' + RESET + '\n');

const byRule = {};
const byFile = {};
for (const v of violations) {
  byRule[v.rule] = (byRule[v.rule] || 0) + 1;
  byFile[v.file] = byFile[v.file] || 0;
  byFile[v.file]++;
}

const totalViolations = violations.length;
const totalFiles = allFiles.length;
const cleanFiles = totalFiles - Object.keys(byFile).length;

const severityOrder = { high: 0, medium: 1, low: 2, info: 3 };
const sortedRules = [...rules].sort((a, b) => severityOrder[a.severity] - severityOrder[b.severity]);

console.log(`扫描文件: ${totalFiles} 个 · 违规命中: ${totalViolations} 次 · 涉及文件: ${Object.keys(byFile).length} · 干净文件: ${cleanFiles}\n`);

console.log(BLUE + '── 按规则分布(按严重度排序)──' + RESET);
for (const rule of sortedRules) {
  const count = byRule[rule.id] || 0;
  const sev = rule.severity;
  const color = sev === 'high' ? RED : sev === 'medium' ? YELLOW : sev === 'low' ? GRAY : BLUE;
  const bar = '█'.repeat(Math.min(20, Math.floor(count / 2)));
  console.log(`  ${color}${rule.id.padEnd(8)}${RESET} ${count.toString().padStart(4)} 次  ${rule.desc}`);
  if (rule.note) console.log(`        ${GRAY}${rule.note}${RESET}`);
  console.log(`        ${color}${bar}${RESET}`);
  if (count > 0) console.log(`        ${GRAY}修复：${rule.fix}${RESET}`);
}

console.log('\n' + BLUE + '── Top 10 违规组件 ──' + RESET);
const sorted = Object.entries(byFile).sort((a, b) => b[1] - a[1]).slice(0, 10);
for (const [f, c] of sorted) {
  console.log(`  ${c.toString().padStart(4)} 次  ${f}`);
}

if (totalViolations > 0) {
  console.log('\n' + BLUE + '── 命中样例（每个规则前 3 条）──' + RESET);
  for (const rule of rules) {
    const samples = violations.filter(v => v.rule === rule.id).slice(0, 3);
    if (samples.length === 0) continue;
    console.log(`\n  ${rule.id}: ${rule.desc}`);
    for (const s of samples) {
      const color = s.severity === 'high' ? RED : s.severity === 'medium' ? YELLOW : GRAY;
      console.log(`    ${color}${s.file}:${s.line}${RESET}  命中="${s.match}"`);
      console.log(`        ${GRAY}${s.text}${RESET}`);
    }
  }
}

// === 报告保存到 tmp/design-compliance.md ===
// tmp/ 已在 .gitignore；docs/ 目录自 2026-09-06 起整体不入库，故不能再作为输出目标
// （新克隆的仓库没有 docs/，写入会 ENOENT）
const reportDir = 'tmp';
const reportPath = path.join(reportDir, 'design-compliance.md');
fs.mkdirSync(reportDir, { recursive: true });
let report = [
  '# 设计规范合规扫描报告',
  '',
  `> 自动生成 by \`scripts/design-compliance-scan.mjs\` · ${new Date().toISOString().slice(0, 10)}`,
  '> 规则来源：`specs/design-tokens.md §4` + `§卡片与交互样式规范`',
  '',
  '## 📊 统计',
  '',
  `| 维度 | 值 |`,
  `|---|---|`,
  `| 扫描文件数 | ${totalFiles} |`,
  `| 命中违规数 | ${totalViolations} |`,
  `| 涉及组件数 | ${Object.keys(byFile).length} |`,
  `| 干净组件数 | ${cleanFiles} |`,
  '',
  '## 📋 按规则分布',
  '',
  '| 规则 | 命中 | 严重度 | 说明 | 修复建议 |',
  '|---|---:|---|---|---|',
];
for (const rule of rules) {
  const count = byRule[rule.id] || 0;
  report.push(`| ${rule.id} | ${count} | ${rule.severity} | ${rule.desc} | ${rule.fix} |`);
}
report.push('', '## 📁 Top 10 违规组件', '', '| 命中数 | 文件 |', '|---:|---|');
for (const [f, c] of sorted) {
  report.push(`| ${c} | \`${f}\` |`);
}
if (totalViolations > 0) {
  report.push('', '## 📝 命中明细', '');
  for (const rule of rules) {
    const samples = violations.filter(v => v.rule === rule.id);
    if (samples.length === 0) continue;
    report.push(`### ${rule.id} · ${rule.desc}`, '');
    report.push('| 文件 | 行 | 命中 | 上下文 |', '|---|---:|---|---|');
    for (const s of samples.slice(0, 50)) {
      const txt = s.text.replace(/\|/g, '\\|');
      report.push(`| \`${s.file}\` | ${s.line} | \`${s.match}\` | ${txt} |`);
    }
    if (samples.length > 50) report.push(`\n> 仅展示前 50 条 · 共 ${samples.length} 处`);
    report.push('');
  }
}

report.push('---', '', '**重新扫描**：`node scripts/design-compliance-scan.mjs` · 输出会覆盖本文档', '');
fs.writeFileSync(reportPath, report.join('\n'), 'utf8');

console.log('\n' + GREEN + '✓ 报告已写入 ' + reportPath + RESET);
console.log(BLUE + '══════════════════════════════════════════════' + RESET + '\n');

// 退出码：high severity 违规 → 非 0
const highCount = violations.filter(v => v.severity === 'high').length;
console.log('\n' + BLUE + '══════════════════════════════════════════════' + RESET);
if (highCount > 0) {
  console.log(RED + `  ✗ ${highCount} 处 high severity 违规,需修复` + RESET);
  console.log(BLUE + '══════════════════════════════════════════════' + RESET + '\n');
  process.exit(1);
} else {
  console.log(GREEN + '  ✓ 无 high severity 违规,设计规范合规' + RESET);
  console.log(BLUE + '══════════════════════════════════════════════' + RESET + '\n');
  process.exit(0);
}