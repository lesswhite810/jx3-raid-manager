// 在 design-tokens.md §1.4 加 "派生 strong 系列" 决策树
import fs from 'node:fs';

const p = 'specs/design-tokens.md';
let s = fs.readFileSync(p, 'utf8');
const EOL = s.includes('\r\n') ? '\r\n' : '\n';

// 找 §1.4 节末尾(下个 ### 之前)
const sec14Start = s.indexOf('### 1.4');
if (sec14Start < 0) { console.log('✗ §1.4 未找到'); process.exit(1); }

// 找下一个 ## 或 ### 开头位置
let secEnd = s.length;
for (const marker of ['\n### ', '\n## ']) {
  const idx = s.indexOf(marker, sec14Start + 6);
  if (idx > 0 && idx < secEnd) secEnd = idx;
}

const before = s.substring(0, secEnd);
const after = s.substring(secEnd);

const addition = [
  '',
  '#### 1.4.1 派生 strong 系列（WCAG AA 4.5:1 小正文专用）',
  '',
  '> 🆕 **v2.2.0（2026-08-23）** 新增 · 决策点 1 方案 C 落地',
  '',
  '每个语义色提供 3 档梯度：',
  '',
  '| 工具类后缀 | 派生值 | 对应 token | vs 白底对比度 | 用途 |',
  '|---|---|---|---:|---|',
  '| DEFAULT | 提案基调色 | `--{success,warning,info}` | 3.60 ~ 4.30:1 | 大字号（≥18px 或 ≥14px bold）· Tag 软底背景 |',
  '| `soft` | 浅色背景 | `--{success,warning,info}-soft` | （背景色） | 浅底背景 / Tag 内底 |',
  '| `strong` | 深色派生 | `--{success,warning,info}-strong` | **6.63 ~ 7.83:1** | ≤18px 正文 · 小标签 · 图标颜色 |',
  '',
  '**色值**（光主题）：',
  '',
  '```css',
  '--success-strong: #3d5e4a;  /* 苔绿派生 */',
  '--warning-strong: #6e5a30;  /* 焦糖派生 */',
  '--info-strong:    #3e546a;  /* 雾蓝派生 */',
  '```',
  '',
  '**色值**（暗主题）：',
  '',
  '```css',
  '--success-strong: #87b29a;  /* 浅一档,与光主题对称 */',
  '--warning-strong: #c9a06b;',
  '--info-strong:    #8eaabd;',
  '```',
  '',
  '#### 1.4.2 何时用 strong 派生（决策树）',
  '',
  '```',
  'className 字号判断',
  '├── text-base / text-lg / text-xl / text-2xl / text-3xl',
  '│   └── 用 DEFAULT（大字号 AA ≥3.0:1 已合规）',
  '├── font-bold + 默认 16px',
  '│   └── 用 DEFAULT（14px bold 算大字号 AA 已合规）',
  '├── text-xs / text-sm / text-[10px] / text-[12px]',
  '│   └── 用 *-strong（小正文 AA 4.5:1 必需）',
  '├── 透明度修饰 text-ds-{sem}/N',
  '│   └── 替换为 text-ds-{sem}-strong（透明度会降低对比度）',
  '├── 小图标 w-[1-4] h-[1-4]',
  '│   └── 替换为 *-strong（图小色需更深补偿）',
  '└── Tag/Pill 内文（bg-ds-{sem}-soft + text-ds-{sem}）',
  '    └── 替换为 *-strong（软底上的文字色对比度不达 AA）',
  '```',
  '',
  '**全仓落地统计（2026-08-23）**：',
  '',
  '- `text-ds-*-strong` 使用：142 行（小正文/标签/图标场景）',
  '- DEFAULT 保留：72 行（大字号 / 大图标 / 数据文件）',
  '- 设计哲学保持：提案基调色不变,仅补充深色派生',
  '',
].join(EOL);

s = before + addition + EOL + after;
fs.writeFileSync(p, s, 'utf8');
console.log('✓ design-tokens.md §1.4.1 + §1.4.2 已加入 strong 派生说明');