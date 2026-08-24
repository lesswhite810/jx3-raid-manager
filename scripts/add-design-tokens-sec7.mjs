// 在 design-tokens.md 末尾追加 §7 合规扫描规则说明
import fs from 'node:fs';

const p = 'specs/design-tokens.md';
let s = fs.readFileSync(p, 'utf8');
const EOL = s.includes('\r\n') ? '\r\n' : '\n';

if (s.includes('### 7. 合规扫描规则说明')) {
  console.log('§7 已存在,跳过');
  process.exit(0);
}

// 找 "---" 分隔符(在 §6 末尾与 "最后更新" 之间)
const idx = s.indexOf(EOL + '---' + EOL);
if (idx < 0) {
  console.log('✗ 未找到 --- 分隔符');
  process.exit(1);
}

const sec7 = [
  '',
  '### 7. 合规扫描规则说明（供维护者参考）',
  '',
  '> 详见 `scripts/design-compliance-scan.mjs`',
  '',
  '| 规则 ID | 严重度 | 检测目标 | 修复方式 |',
  '|---|---|---|---|',
  '| R1 | high | `shadow-md / lg / xl / 2xl` | 改为 `shadow-ds-stack / -modal` |',
  '| R2 | high | `linear-gradient / radial-gradient` | 用单色 + shadow-ds-* |',
  '| R3 | medium | `duration-500/700/1000` | 改为 `duration-ds-fast/base/slow` |',
  '| R4 | medium | `ease-bounce / ease-spring` | 改为 `ease-ds-ease` |',
  '| R5 | low | emoji 图标（🎮⚔🩸⏰👁🏆💎💰🔍✨🐛📊 等） | 用 Lucide React |',
  '| R6 | low | 连续感叹号 `!!!` / `！！！` | AGENTS.md §4.4 |',
  '| R7 | info | 红色直接用作文字色 | 合规：删除按钮 / 必填星号 / 错误提示 |',
  '| R7-info | info | red 系总用量 | 全部用于合规场景 |',
  '| R8 | low | `font-sans` 显式调用 | 展示位用 `font-display / font-body` |',
  '',
  '**退出码**：发现 high severity 违规时 `exit 1`（CI 友好）',
  '',
].join(EOL);

// 插在 --- 之前
const before = s.substring(0, idx);
const after = s.substring(idx);
s = before + sec7 + EOL + after.substring(EOL.length);
fs.writeFileSync(p, s, 'utf8');
console.log('✓ §7 已插入');