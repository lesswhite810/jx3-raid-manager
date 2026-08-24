// 一次性迁移脚本：把 Tailwind 默认 emerald-* / amber-* 替换为提案 token
// 着色规则：50/100/200/800/900 → soft;300/400/500/600/700 → DEFAULT
// 透明度修饰 /NN 直接透传（Tailwind 3.x 支持 solid + 透明度）
import fs from 'node:fs';
import path from 'node:path';

const targets = [
  'components',
  'src',
  'App.tsx',
  'index.html',
];

function walk(dir, list = []) {
  if (!fs.existsSync(dir)) return list;
  const stat = fs.statSync(dir);
  if (stat.isFile()) { list.push(dir); return list; }
  for (const e of fs.readdirSync(dir)) {
    const p = path.join(dir, e);
    const s = fs.statSync(p);
    if (s.isDirectory()) walk(p, list);
    else if (/\.(tsx?|jsx?|html|css)$/.test(e)) list.push(p);
  }
  return list;
}

const files = targets.flatMap(t => walk(t));
console.log(`扫描到 ${files.length} 个文件`);

// 映射表：正则匹配 → 替换
// 关键规则：
// 1. emerald-50/100/200/800/900 → soft
// 2. emerald-300/400/500/600/700 → DEFAULT
// 3. 透明度修饰 /NN 跟随
function buildReplacer(hue, semanticName) {
  const softShades = ['50', '100', '200', '800', '900'];
  const defaultShades = ['300', '400', '500', '600', '700'];
  return (input) => {
    let out = input;
    // 处理 soft shades
    for (const shade of softShades) {
      const re = new RegExp(
        String.raw`\b(bg|text|border|ring|placeholder|outline|fill|stroke|via|to|from)-${hue}-${shade}(\/\d+)?\b`,
        'g'
      );
      out = out.replace(re, (m, prefix, opacity) =>
        opacity
          ? `${prefix}-ds-${semanticName}-soft${opacity}`
          : `${prefix}-ds-${semanticName}-soft`
      );
    }
    // 处理 default shades
    for (const shade of defaultShades) {
      const re = new RegExp(
        String.raw`\b(bg|text|border|ring|placeholder|outline|fill|stroke|via|to|from)-${hue}-${shade}(\/\d+)?\b`,
        'g'
      );
      out = out.replace(re, (m, prefix, opacity) =>
        opacity
          ? `${prefix}-ds-${semanticName}${opacity}`
          : `${prefix}-ds-${semanticName}`
      );
    }
    // 兜底：dark: 前缀处理（Tailwind v3.4 支持 dark: utility）
    // 由于上面正则匹配包含 dark 修饰符的整个 className 中的 hue-shade 部分也会被替换
    return out;
  };
}

const replaceEmerald = buildReplacer('emerald', 'success');
const replaceAmber = buildReplacer('amber', 'warning');

// 文件级白名单：保留脚本/样式文件名不被换行符干扰
const skipFiles = new Set([
  'index.css',
  'tailwind.config.js',
  'package.json',
  'package-lock.json',
]);

let totalChanges = 0;
const summary = {};

for (const f of files) {
  const base = path.basename(f);
  if (skipFiles.has(base)) continue;
  const original = fs.readFileSync(f, 'utf8');
  let patched = original;
  // 先 emerald 再 amber（避免 conflict）
  patched = replaceEmerald(patched);
  patched = replaceAmber(patched);
  if (patched !== original) {
    const diffBytes = patched.length - original.length;
    fs.writeFileSync(f, patched, 'utf8');
    const changes = (patched.match(/ds-(success|warning)(-soft)?(\/\d+)?/g) || []).length;
    summary[f] = changes;
    totalChanges += changes;
    console.log(`  ${f}: ${changes} 处匹配`);
  }
}

console.log('');
console.log(`✓ 完成 · 共 ${totalChanges} 处重命名（跨 ${Object.keys(summary).length} 个文件）`);
if (totalChanges === 0) {
  console.log('⚠️ 没有产生任何替换，请检查正则与文件列表');
}
