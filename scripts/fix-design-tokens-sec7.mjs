// 把 §7 从错误位置(§1.7 节)移动到正确位置(§6 之后)
import fs from 'node:fs';

const p = 'specs/design-tokens.md';
let s = fs.readFileSync(p, 'utf8');
const EOL = s.includes('\r\n') ? '\r\n' : '\n';

// 找到 §7 当前位置(L44, 在 §1.7 节里)
const wrongIdx = s.indexOf('### 7. 合规扫描规则说明（供维护者参考）');
if (wrongIdx < 0) {
  console.log('§7 不在 §1.7 节里,可能已正确,跳过');
  process.exit(0);
}

// 删除从 §7 开始到下一个 --- 之前的所有内容
const sec7EndIdx = s.indexOf(EOL + '---' + EOL, wrongIdx);
if (sec7EndIdx < 0) {
  console.log('✗ 找不到 §7 结束位置');
  process.exit(1);
}
// 删除内容包括中间的换行
const before = s.substring(0, wrongIdx);
const after = s.substring(sec7EndIdx + EOL.length);
let s2 = before + after;

// 把删除的内容放到正确位置:§6 末尾与"最后更新"之间
const sec7Content = [
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

// 找到 "最后更新" 行
const lastUpdatedIdx = s2.indexOf('**最后更新**');
if (lastUpdatedIdx < 0) {
  console.log('✗ 找不到最后更新');
  process.exit(1);
}

// 在 §6 末尾(--- 后)和 最后更新 之前插入 §7
const beforeLast = s2.substring(0, lastUpdatedIdx);
const fromLast = s2.substring(lastUpdatedIdx);
s2 = beforeLast.trimEnd() + EOL + sec7Content + EOL + fromLast;

fs.writeFileSync(p, s2, 'utf8');
console.log('✓ §7 已从错误位置移除,重新插入到 §6 之后');