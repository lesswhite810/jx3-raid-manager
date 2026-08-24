// C2 试点:在 index.css 与 tailwind.config.js 加 3 个 *-strong 派生 token
// 仅试点,不动组件类名(由后续 C2.2 替换 5 处)
import fs from 'node:fs';

// === 1) index.css:光主题(:root) ===
const idxPath = 'index.css';
let idx = fs.readFileSync(idxPath, 'utf8');
const idxEOL = idx.includes('\r\n') ? '\r\n' : '\n';

// 在 --success-soft 后面加 --success-strong;同理 warning / info
// 光主题
idx = idx.replace(
  /(\s+--success-soft: #e3ebe5;)/,
  '$1' + idxEOL + '        --success-strong: #3d5e4a;  /* 决策点 1 派生：用于 ≤18px 正文场景（白底 7.25:1） */'
);
idx = idx.replace(
  /(\s+--warning-soft: #f0e8d8;)/,
  '$1' + idxEOL + '        --warning-strong: #6e5a30;  /* 决策点 1 派生：用于 ≤18px 正文场景（白底 6.63:1） */'
);
idx = idx.replace(
  /(\s+--info-soft: #dfe5ea;)/,
  '$1' + idxEOL + '        --info-strong: #3e546a;     /* 决策点 1 派生：用于 ≤18px 正文场景（白底 7.83:1） */'
);

// 暗主题
idx = idx.replace(
  /(\s+--success-soft: #1a2520;)/,
  '$1' + idxEOL + '        --success-strong: #87b29a;  /* 暗主题:浅一档（深底 → 浅字） */'
);
idx = idx.replace(
  /(\s+--warning-soft: #2e261a;)/,
  '$1' + idxEOL + '        --warning-strong: #c9a06b;  /* 暗主题:浅一档 */'
);
idx = idx.replace(
  /(\s+--info-soft: #1a2229;)/,
  '$1' + idxEOL + '        --info-strong: #8eaabd;     /* 暗主题:浅一档 */'
);

fs.writeFileSync(idxPath, idx, 'utf8');
console.log('✓ index.css 3 个 *-strong token 已加(光+暗)');

// === 2) tailwind.config.js:暴露 Tailwind 工具类 ===
const twPath = 'tailwind.config.js';
let tw = fs.readFileSync(twPath, 'utf8');
const twEOL = tw.includes('\r\n') ? '\r\n' : '\n';

// 在 ds-success / ds-warning / ds-info 配置附近找插入点
// 假设结构:'ds-success': 'rgb(var(--success) / <alpha-value>)'
// 在每行后插入 ds-{success,warning,info}-strong
const addStrong = [
  {
    find: "'ds-success': 'rgb(var(--success) / <alpha-value>)',",
    insert: twEOL + "        'ds-success-strong': 'rgb(var(--success-strong) / <alpha-value>)',",
  },
  {
    find: "'ds-warning': 'rgb(var(--warning) / <alpha-value>)',",
    insert: twEOL + "        'ds-warning-strong': 'rgb(var(--warning-strong) / <alpha-value>)',",
  },
  {
    find: "'ds-info': 'rgb(var(--info) / <alpha-value>)',",
    insert: twEOL + "        'ds-info-strong': 'rgb(var(--info-strong) / <alpha-value>)',",
  },
];

let twChanges = 0;
const updates = [
  { keyword: "'ds-success': {", insertAfter: "          soft: 'var(--success-soft)',", strongKey: 'success-strong' },
  { keyword: "'ds-warning': {", insertAfter: "          soft: 'var(--warning-soft)',", strongKey: 'warning-strong' },
  { keyword: "'ds-info': {", insertAfter: "          soft: 'var(--info-soft)',", strongKey: 'info-strong' },
];
for (const u of updates) {
  const fullInsert = u.insertAfter + twEOL + '          strong: \'var(--' + u.strongKey + ')\',';
  if (tw.includes(u.insertAfter) && !tw.includes("var(--" + u.strongKey + ")")) {
    tw = tw.replace(u.insertAfter, fullInsert);
    twChanges++;
  }
}

fs.writeFileSync(twPath, tw, 'utf8');
console.log(`✓ tailwind.config.js ${twChanges} 处 *-strong 工具类已暴露`);

// === 3) 验证 ===
console.log('');
console.log('═══ 验证 ═══');
const checkIdx = (() => {
  const s = fs.readFileSync(idxPath, 'utf8');
  return {
    light_success_strong: s.includes('--success-strong: #3d5e4a'),
    light_warning_strong: s.includes('--warning-strong: #6e5a30'),
    light_info_strong: s.includes('--info-strong: #3e546a'),
    dark_success_strong: s.includes('--success-strong: #87b29a'),
    dark_warning_strong: s.includes('--warning-strong: #c9a06b'),
    dark_info_strong: s.includes('--info-strong: #8eaabd'),
  };
})();
const checkTw = (() => {
  const s = fs.readFileSync(twPath, 'utf8');
  return {
    ds_success_strong: s.includes("'ds-success-strong':"),
    ds_warning_strong: s.includes("'ds-warning-strong':"),
    ds_info_strong: s.includes("'ds-info-strong':"),
  };
})();
console.log('index.css:');
for (const [k, v] of Object.entries(checkIdx)) console.log(`  ${v ? '✓' : '✗'} ${k}`);
console.log('tailwind.config.js:');
for (const [k, v] of Object.entries(checkTw)) console.log(`  ${v ? '✓' : '✗'} ${k}`);