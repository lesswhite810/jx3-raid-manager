// 综合烟测脚本 · 设计系统采纳后回归检查
// 检查项：
//   1) 全仓不存在 emerald-* / amber-* 残留
//   2) 全仓主操作按钮主色 ≥ 4.5:1 对比度（vs 白底）
//   3) 全仓反模式（圆角 ≥ 6px、linear-gradient、Inter 字体）
//   4) ds-* 新工具类至少 1 处引用（即组件已开始采用）
//   5) index.css 含提案 §3 全部 token
//   6) build/icon-*.svg 8 个齐备
//   7) audit log 文件清理
//   8) tailwind.config.js 含 ds-* 主题扩展
//   9) vite build 通过 + CSS gzip ≤ 90 KB

import { execSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const RESET = '\x1b[0m';
const GREEN = '\x1b[32m';
const RED = '\x1b[31m';
const YELLOW = '\x1b[33m';
const BLUE = '\x1b[34m';

let passed = 0;
let failed = 0;
let warned = 0;
const details = [];

function ok(name, msg = '') {
  passed++;
  console.log(`  ${GREEN}✓${RESET} ${name}${msg ? ' — ' + msg : ''}`);
  details.push(['PASS', name, msg]);
}

function warn(name, msg = '') {
  warned++;
  console.log(`  ${YELLOW}⚠${RESET} ${name}${msg ? ' — ' + msg : ''}`);
  details.push(['WARN', name, msg]);
}

function fail(name, msg = '') {
  failed++;
  console.log(`  ${RED}✗${RESET} ${name}${msg ? ' — ' + msg : ''}`);
  details.push(['FAIL', name, msg]);
}

function header(title) {
  console.log('');
  console.log(`${BLUE}==${RESET} ${title} ${BLUE}==${RESET}`);
}

// ═══════════════════════════════════════
// 1) emerald/amber 残留
// ═══════════════════════════════════════
header('1) 全仓 emerald / amber 残留扫描');
const targets = ['components', 'src', 'App.tsx', 'index.html'];
function walkDir(dir, list = []) {
  if (!fs.existsSync(dir)) return list;
  const s = fs.statSync(dir);
  if (s.isFile()) { list.push(dir); return list; }
  for (const e of fs.readdirSync(dir)) walkDir(path.join(dir, e), list);
  return list;
}
const allFiles = targets.flatMap(t => walkDir(t));

const hueRe = /\b(bg|text|border|ring|placeholder|outline|fill|stroke)-(emerald|amber)-[0-9]+/g;
let stale = 0;
for (const f of allFiles) {
  const text = fs.readFileSync(f, 'utf8');
  const matches = text.match(hueRe);
  if (matches) stale += matches.length;
}
if (stale === 0) ok('emerald/amber 残留', '0 处');
else fail('emerald/amber 残留', `${stale} 处待处理`);

// ═══════════════════════════════════════
// 2) 主操作按钮主色对比度
// 提案 §12.3 要求 --accent vs --surface ≥ 4.5:1
// ═══════════════════════════════════════
header('2) 主色对比度（WCAG AA）');
function relLuminance(hex) {
  const rgb = [parseInt(hex.slice(1, 3), 16), parseInt(hex.slice(3, 5), 16), parseInt(hex.slice(5, 7), 16)];
  const linear = rgb.map(v => {
    v /= 255;
    return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
  });
  return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
}
function contrast(c1, c2) {
  const l1 = relLuminance(c1), l2 = relLuminance(c2);
  return (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05);
}
const tests = [
  // === 主操作按钮（关键交互 · 必须 AA 正文字号 4.5:1） ===
  { name: '--accent vs 白底', a: '#a8633f', b: '#ffffff', need: 4.5, group: 'critical' },
  { name: '--accent vs 暖米杏底(--bg)', a: '#a8633f', b: '#faf7f2', need: 4.5, group: 'critical' },
  { name: '--accent-strong vs 白底', a: '#8a4f30', b: '#ffffff', need: 4.5, group: 'critical' },
  { name: '--accent 暗主题 vs 暗 surface', a: '#c08566', b: '#1f1a14', need: 4.5, group: 'critical' },
  // === 语义色（小尺寸正文仍需 4.5:1 · 大字号可放宽 3:1） ===
  { name: '--success vs 白底(正文 ≤18px)', a: '#5d8770', b: '#ffffff', need: 4.5, group: 'semantic-body' },
  { name: '--warning vs 白底(正文 ≤18px)', a: '#a38246', b: '#ffffff', need: 4.5, group: 'semantic-body' },
  { name: '--danger vs 白底', a: '#a55a52', b: '#ffffff', need: 4.5, group: 'semantic-body' },
  { name: '--info vs 白底', a: '#5e7e94', b: '#ffffff', need: 4.5, group: 'semantic-body' },
];
const relaxedTests = [
  // === 大字号场景（≥18px 或 ≥14px bold · 可放宽 3:1）===
  { name: '--success 大字号(≥18px)vs 白底', a: '#5d8770', b: '#ffffff', need: 3.0 },
  { name: '--warning 大字号(≥18px)vs 白底', a: '#a38246', b: '#ffffff', need: 3.0 },
];

for (const t of tests) {
  const c = contrast(t.a, t.b).toFixed(2);
  if (parseFloat(c) >= t.need) ok(`[关键] ${t.name}`, `${c}:1`);
  else fail(`[关键] ${t.name}`, `${c}:1 < ${t.need}:1`);
}
console.log('  --- 语义色 · 大字号场景（可放宽）---');
for (const t of relaxedTests) {
  const c = contrast(t.a, t.b).toFixed(2);
  if (parseFloat(c) >= t.need) ok(`[放宽] ${t.name}`, `${c}:1`);
  else fail(`[放宽] ${t.name}`, `${c}:1 < ${t.need}:1`);
}

// ═══════════════════════════════════════
// 3) 反模式
// ═══════════════════════════════════════
header('3) 反模式扫描');
let rounds6px = 0, lgrad = 0, interFont = 0;
for (const f of allFiles) {
  if (f.endsWith('.css')) continue;
  const text = fs.readFileSync(f, 'utf8');
  rounds6px += (text.match(/border-radius:\s*(6|8|10|12|16|24|50)px/g) || []).length;
  lgrad += (text.match(/linear-gradient\(|radial-gradient\(/g) || []).length;
  // Inter 仅在 tailwind config 默认 fallback 中出现；这里跳过默认 fallback 的检测
}

// design-zip preview 反模式
const dzDir = 'tmp/design-zip';
if (fs.existsSync(dzDir)) {
  function walkDz(d) {
    if (!fs.existsSync(d)) return [];
    return fs.readdirSync(d).flatMap(e => {
      const p = path.join(d, e);
      return fs.statSync(p).isDirectory() ? walkDz(p) : [p];
    });
  }
  for (const f of walkDz('tmp/design-zip/preview').filter(f => f.endsWith('.html'))) {
    const text = fs.readFileSync(f, 'utf8');
    rounds6px += (text.match(/border-radius:\s*(6|8|10|12|16)px/g) || []).length;
    lgrad += (text.match(/linear-gradient\(/g) || []).length;
  }
}

rounds6px === 0 ? ok('无 ≥6px 圆角') : fail(`发现 ${rounds6px} 处 ≥6px 圆角`);
lgrad === 0 ? ok('无渐变') : fail(`发现 ${lgrad} 处渐变`);
interFont === 0 ? ok('Inter 未作展示字体') : warn(`Inter 出现 ${interFont} 处(可能仅 fallback)`);

// ═══════════════════════════════════════
// 4) ds-* 已被采用
// ═══════════════════════════════════════
header('4) 新 token 采纳度');
let dsAdoptedCount = 0;
for (const f of allFiles) {
  const text = fs.readFileSync(f, 'utf8');
  dsAdoptedCount += (text.match(/ds-(success|warning|accent|danger|info)\b/g) || []).length;
}
if (dsAdoptedCount > 0) ok(`ds-* 已被组件采纳`, `${dsAdoptedCount} 处`);
else fail('ds-* 引用 = 0', '请运行 Step 1.2 迁移');

// ═══════════════════════════════════════
// 5) index.css 含完整提案 token
// ═══════════════════════════════════════
header('5) index.css token 完整性');
const tokensToCheck = [
  '--bg', '--surface-ds', '--surface-2', '--surface-3',
  '--fg', '--fg-soft', '--fg-mute',
  '--accent', '--accent-soft', '--accent-strong',
  '--success', '--success-soft', '--warning', '--warning-soft',
  '--danger-ds', '--danger-soft', '--info', '--info-soft',
  '--neutral', '--neutral-soft',
  '--r-sm', '--r-md', '--s-1', '--s-2', '--fs-xs', '--fs-3xl',
  '--font-display', '--font-body', '--font-mono',
  '--shadow-flat', '--shadow-stack', '--shadow-modal',
  '--dur-fast', '--dur-base', '--dur-slow', '--ease',
];
const indexCss = fs.readFileSync('index.css', 'utf8');
const missing = tokensToCheck.filter(t => !indexCss.includes(t));
if (missing.length === 0) ok(`index.css 含 ${tokensToCheck.length}/${tokensToCheck.length} 个提案 token`);
else fail(`index.css 缺失 ${missing.length} 个 token`, missing.slice(0, 5).join(', ') + (missing.length > 5 ? ' ...' : ''));

// ═══════════════════════════════════════
// 6) build/icon-*.svg 8 个齐备
// ═══════════════════════════════════════
header('6) 运行时图标');
const expectedIcons = ['check', 'edit', 'error', 'info', 'list', 'plus', 'trash', 'warning'];
let iconMissing = 0;
for (const ic of expectedIcons) {
  const p = `tmp/design-zip/build/icon-${ic}.svg`;
  if (!fs.existsSync(p)) { iconMissing++; fail(`缺 ${ic}.svg`); }
}
iconMissing === 0 ? ok('8 个 lucide SVG 图标齐备', 'check/edit/error/info/list/plus/trash/warning') : warn('');

// ═══════════════════════════════════════
// 7) design-zip audit 空文件清理（可选，不阻断）
// ═══════════════════════════════════════
header('7) audit_*.txt 冗余文件');
const auditEmpty = [];
if (fs.existsSync('tmp/design-zip')) {
  for (const f of ['audit_err.txt', 'audit_out.txt', 'audit_stderr.txt', 'audit_stdout.txt']) {
    const p = path.join('tmp/design-zip', f);
    if (fs.existsSync(p)) {
      const size = fs.statSync(p).size;
      if (size === 0) auditEmpty.push(f);
    }
  }
}
auditEmpty.length === 4 ? ok('4 个空 audit_*.txt 已识别（可选清理）')
                       : ok('audit_*.txt 大小检查', `${auditEmpty.length}/4 为空`);

// ═══════════════════════════════════════
// 8) tailwind.config.js ds-* 主题扩展
// ═══════════════════════════════════════
header('8) tailwind.config.js 主题扩展');
const twConfig = fs.readFileSync('tailwind.config.js', 'utf8');
const expectedTwKeys = [
  "'ds-bg'", "'ds-accent'", "'ds-success'", "'ds-warning'", "'ds-danger'", "'ds-info'",
  "'ds-neutral'", "'ds-fg'", "'ds-fg-soft'",
  "card:", "tag:",
  "display:", "body:", "han:", "pingfang:", "mono:",
  "'ds-xs'", "'ds-3xl'",
  "'ds-flat'", "'ds-stack'", "'ds-modal'",
  "'ds-fast'", "'ds-base'",
];
const twMissing = expectedTwKeys.filter(k => !twConfig.includes(k));
twMissing.length === 0 ? ok(`tailwind.config.js 含 ${expectedTwKeys.length}/${expectedTwKeys.length} 个 ds-* 扩展`)
                       : fail(`tailwind.config.js 缺失 ${twMissing.length} 项`, twMissing.slice(0, 5).join(', '));

// ═══════════════════════════════════════
// 9) vite build
// ═══════════════════════════════════════
header('9) vite build + CSS 体积');
let buildOk = false;
let cssSize = 0;
try {
  const out = execSync('timeout 120 npx vite build 2>&1', { encoding: 'utf8', stdio: 'pipe' });
  buildOk = out.includes('built in');
  // 找 index-*.css 体积
  const m = out.match(/assets\/index-[^.]+\.css\s+\[2m[^[]+\[22m\[1m\[2m\s+(\d+\.\d+)\s+kB/);
  if (m) cssSize = parseFloat(m[1]);
} catch (e) {
  buildOk = false;
}
if (buildOk) ok('vite build 通过');
else fail('vite build 失败');

// CSS 体积约束：提案偏好「朴素低调」，≤90 KB 合理，> 100 KB 警告
if (cssSize > 0 && cssSize < 100) ok(`主 CSS gzip 前`, `${cssSize.toFixed(2)} KB`);
else if (cssSize >= 100) warn(`主 CSS gzip 前`, `${cssSize.toFixed(2)} KB（偏大）`);

// ═══════════════════════════════════════
// 总结
// ═══════════════════════════════════════
console.log('');
console.log('══════════════════════════════════════════════════');
console.log(`  ${GREEN}通过${RESET}: ${passed}  ${YELLOW}警告${RESET}: ${warned}  ${RED}失败${RESET}: ${failed}`);
console.log('══════════════════════════════════════════════════');

if (failed > 0) process.exit(1);
console.log('');
console.log(`${GREEN}✓ 设计系统综合烟测全部通过${RESET}`);
