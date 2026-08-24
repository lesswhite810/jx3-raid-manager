// One-shot helper: 在 index.css 中追加设计系统提案 token
// 用法: node scripts/patch-index-css.mjs
// 安全：UTF-8 + 保持 CRLF 行尾与现有格式
import fs from 'node:fs';
import path from 'node:path';

const filePath = path.resolve('index.css');
const original = fs.readFileSync(filePath, 'utf8');
const EOL = original.includes('\r\n') ? '\r\n' : '\n';
const lightHook = `        --radius: 0.5rem;${EOL}    }${EOL}`;
const darkHook = `        --chart-expense: 251 191 36;${EOL}        /* amber-400 */${EOL}    }${EOL}`;

const LIGHT_TOKENS = [
  '',
  '        /* ============================================================',
  '         * 设计系统提案 token（2026-08-23 纳入 · 对应 tmp/design-zip/DESIGN.md §3 / colors_and_type.css §3）',
  '         * 命名遵循 --{role}: {value};与 Tailwind 默认调色板无冲突',
  '         * ============================================================ */',
  '',
  '        /* —— Surface 表面层级 —— */',
  '        --bg: #faf7f2;            /* 主背景：暖米杏（呼应江湖纸笜） */',
  '        --surface-ds: #ffffff;        /* 一级卡片 */',
  '        --surface-2: #f3efe8;        /* 二级面板 */',
  '        --surface-3: #ede7dc;        /* 三级 / 分隔带 / 禁用底 */',
  '        --border-ds: #e3dccc;        /* 默认描边 */',
  '        --border-strong: #d4c8b3;   /* 选中描边 */',
  '',
  '        /* —— Foreground 文本层级 —— */',
  '        --fg: #1f1a14;               /* 主文本：黑棕 */',
  '        --fg-soft: #4a4339;          /* 次级文本 */',
  '        --fg-mute: #8a7f70;          /* 三级文本 / 占位 */',
  '        --fg-inverse: #ffffff;       /* 反色文本 */',
  '',
  '        /* —— Accent 主强调（沉香褐 · 2026-08-23 降饱和） —— */',
  '        --accent: #a8633f;           /* 江湖印章 */',
  '        --accent-soft: #f3e8df;      /* hover 底 */',
  '        --accent-strong: #8a4f30;    /* 按下 / 焦点描边 */',
  '',
  '        /* —— Semantic 语义色（同步降饱和） —— */',
  '        --success: #5d8770;          /* 苔绿：完成 / 入账 */',
  '        --success-soft: #e3ebe5;',
  '        --warning: #a38246;          /* 焦糖：待确认 / 提醒 */',
  '        --warning-soft: #f0e8d8;',
  '        --danger-ds: #a55a52;            /* 砖红：失败 / 删除 */',
  '        --danger-soft: #ecdfdc;',
  '        --info: #5e7e94;             /* 雾蓝：信息 / 链接 */',
  '        --info-soft: #dfe5ea;',
  '        --neutral: #6a6258;          /* 中性 Tag */',
  '        --neutral-soft: #ebe6db;',
  '',
  '        /* —— Radius 圆角（严格 2 档） —— */',
  '        --r-sm: 2px;                 /* Tag / Chip / 按钮 */',
  '        --r-md: 4px;                 /* 卡片 / 输入框 / Toast */',
  '',
  '        /* —— Spacing 间距（4 倍数） —— */',
  '        --s-1: 4px;',
  '        --s-2: 8px;',
  '        --s-3: 12px;',
  '        --s-4: 16px;',
  '        --s-5: 24px;',
  '        --s-6: 32px;',
  '        --s-7: 48px;',
  '        --s-8: 64px;',
  '',
  '        /* —— Type 字号（major-third · 1.25 倍） —— */',
  '        --fs-xs: 12px;',
  '        --fs-sm: 13px;',
  '        --fs-md: 14px;',
  '        --fs-lg: 16px;',
  '        --fs-xl: 20px;',
  '        --fs-2xl: 24px;',
  '        --fs-3xl: 30px;',
  '        --fs-4xl: 36px;',
  '',
  '        --lh-tight: 1.25;',
  '        --lh-base: 1.5;',
  '        --lh-loose: 1.7;',
  '',
  '        /* —— Type family 字体配（system fallback，不下载 Web Font） —— */',
  '        --font-display: "Source Han Serif SC", "Noto Serif SC", "宋体", "Songti SC", serif;',
  '        --font-body: "PingFang SC", "Microsoft YaHei", "Hiragino Sans GB", system-ui, sans-serif;',
  '        --font-mono: "SF Mono", "JetBrains Mono", "Menlo", Consolas, monospace;',
  '',
  '        /* —— Shadow 阴影（低强度 · 朴素） —— */',
  '        --shadow-flat: 0 1px 2px rgba(31, 26, 20, 0.04);     /* 卡片悬浮 */',
  '        --shadow-stack: 0 2px 6px rgba(31, 26, 20, 0.06);    /* 弹窗 */',
  '        --shadow-modal: 0 6px 16px rgba(31, 26, 20, 0.08);   /* Toast / 抽屉 */',
  '',
  '        /* —— Motion 动效 —— */',
  '        --dur-fast: 120ms;',
  '        --dur-base: 180ms;',
  '        --dur-slow: 280ms;',
  '        --ease: cubic-bezier(0.2, 0.6, 0.2, 1);',
  '    }',
].join('\n');

const DARK_TOKENS = [
  '',
  '        /* —— 设计系统提案 token · 暗主题覆写（colors_and_type.css §12 / DESIGN.md §12.4） —— */',
  '        --bg: #15110d;',
  '        --surface-ds: #1f1a14;',
  '        --surface-2: #2a241c;',
  '        --surface-3: #352d22;',
  '        --border-ds: #3a3127;',
  '        --border-strong: #4a3f31;',
  '',
  '        --fg: #f3ede2;',
  '        --fg-soft: #c9c0b0;',
  '        --fg-mute: #8a7f70;',
  '        --fg-inverse: #15110d;',
  '',
  '        --accent: #c08566;           /* 陶土暖橙（暗底上提亮） */',
  '        --accent-soft: #2e1d14;',
  '        --accent-strong: #d49b7c;',
  '',
  '        --success: #87b29a;',
  '        --success-soft: #1a2520;',
  '        --warning: #c9a06b;',
  '        --warning-soft: #2e261a;',
  '        --danger-ds: #c87872;',
  '        --danger-soft: #2a1d1c;',
  '        --info: #8eaabd;',
  '        --info-soft: #1a2229;',
  '        --neutral: #a89e8c;',
  '        --neutral-soft: #2a2520;',
  '',
  '        --shadow-flat: 0 1px 2px rgba(0, 0, 0, 0.20);     /* 暗底投影 ≥20% */',
  '        --shadow-stack: 0 2px 6px rgba(0, 0, 0, 0.28);',
  '        --shadow-modal: 0 6px 16px rgba(0, 0, 0, 0.36);',
  '    }',
].join('\n');

let patched = original;
let changed = 0;
if (patched.includes(lightHook)) {
  patched = patched.replace(lightHook, LIGHT_TOKENS + EOL);
  changed++;
} else {
  console.error('未找到 lightHook（--radius 锚点），拒绝写入');
  process.exit(1);
}
if (patched.includes(darkHook)) {
  patched = patched.replace(darkHook, DARK_TOKENS + EOL);
  changed++;
} else {
  console.error('未找到 darkHook（amber-400 锚点），拒绝写入');
  process.exit(1);
}

if (changed === 2) {
  fs.writeFileSync(filePath, patched, 'utf8');
  console.log(`✓ index.css 已追加设计系统提案 token（${(patched.length - original.length)} 字节）`);
} else {
  console.error('只命中了部分锚点，跳过写入');
  process.exit(1);
}
