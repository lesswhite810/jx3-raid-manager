// 同步 design-tokens.md 状态(2026-08-23)
//
// ⚠️ 一次性补丁脚本，已于 2026-08-23 执行完毕，**不幂等**：
//    重复运行会把 §4 顶部的「扫描工具」引用块重复插入（其匹配条件 `oldH4` 只锚定标题行，
//    永远成立）。保留仅作留档，**勿直接重跑**；若确需执行，请先核对 design-tokens.md
//    是否已包含目标内容。输出路径口径已于 2026-09-11 由 docs/ 改为 tmp/。
import fs from 'node:fs';

const p = 'specs/design-tokens.md';
let s = fs.readFileSync(p, 'utf8');
const EOL = s.includes('\r\n') ? '\r\n' : '\n';
const log = [];

// §4 顶部
const oldH4 = '### 4. 设计原则合规清单（新组件必过）' + EOL;
const newH4 = [
  '### 4. 设计原则合规清单（新组件必过）',
  '',
  '> 📋 **扫描工具**：`node scripts/design-compliance-scan.mjs` · 输出 `tmp/design-compliance.md`',
  '> 📅 **最近扫描**：2026-08-23 · 39 个文件 · **0 处 high severity 违规**（合规）',
  '> 📅 **最近批量修复**：2026-08-23 · 56 处 `shadow-md/lg/xl/2xl` → `shadow-ds-stack/modal`',
  '',
].join(EOL);
if (s.includes(oldH4)) {
  s = s.replace(oldH4, newH4);
  log.push('§4 顶部');
} else log.push('✗ §4 顶部');

// §4 阴影项
const oldLine = '- [ ] 阴影用 `shadow-ds-*`，不用 `shadow-md / shadow-lg / shadow-2xl`' + EOL;
const newLine = '- [x] 阴影用 `shadow-ds-*`，不用 `shadow-md / shadow-lg / shadow-2xl` ← 2026-08-23 已批量合规（56 处 → `shadow-ds-stack/-modal`）' + EOL;
if (s.includes(oldLine)) {
  s = s.replace(oldLine, newLine);
  log.push('§4 阴影打勾');
} else log.push('✗ §4 阴影');

// §5 fonts/ 完成
const oldFonts = '- ❌ `fonts/`（思源宋体 / 苹方 / SF Mono 文件） —— 尚未补；现有 `index.css` 字体配走 system fallback，可不补' + EOL;
const newFonts = '- ✅ `fonts/`（思源宋体 / 苹方 / SF Mono 文件） —— 不补，走 system fallback；当前 `index.css` 配置合规' + EOL;
if (s.includes(oldFonts)) {
  s = s.replace(oldFonts, newFonts);
  log.push('§5 fonts/ 完成');
} else log.push('✗ §5 fonts/');

// §6 表格化(已运行过)
const oldLines = [
  '1. **accent 颜色统一**（选项 A vs 选项 B，见 §1.3）',
  '2. **warning/success hex 与 emerald-600/amber-600 关系**（选项 A vs 选项 B，见 §1.4）',
  '3. **`tmp/design-zip/AUDIT-REPORT.md`** 中提到的 3 处反模式违例（`preview/applied-surfaces.html` 与 `brand-assets.html` 的 8px 圆角 / 渐变）',
  '4. **`colors_and_type.css` 注释块里的 `--role-*` 字面值** —— 是有意保留的废弃说明，**不应作为真正 token 使用**；建议修复包内自带的审计脚本以过滤注释（见审计报告 §A1）',
];
const oldBlock = oldLines.join(EOL);
if (s.includes(oldBlock)) {
  // §6 旧版未替换,跑替换
  const newBlock = [
    '| 1 | accent 颜色统一 | ✅ 已落地 | `--primary-base` 改为 沉香褐（`168 99 63`）· v2.2.0（2026-08-23） |',
    '| 2 | warning/success 语义统一 | ✅ 已落地 | `emerald-* / amber-*` → `ds-success / ds-warning`（764 处替换）· v2.2.0 |',
    '| 3 | AUDIT-REPORT 反模式违例 | ✅ 已修复 | `preview/applied-surfaces.html` 2 处 8px 圆角 + `brand-assets.html` 3 处 linear-gradient · v2.2.0 |',
    '| 4 | `--role-*` 注释 + 审计脚本误报 | ✅ 已修复 | `README.md §5.3` grep 改为 `^\\s*--role-` 行首匹配 · v2.2.0 |',
  ].join(EOL);
  s = s.replace(oldBlock, newBlock);
  log.push('§6 表格化');
} else {
  log.push('§6 已表格化(跳过)');
}

fs.writeFileSync(p, s, 'utf8');
console.log(log.join('\n'));