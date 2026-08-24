// 一次性脚本：迁移 data/kungfu_data.json 中的 emerald/amber 类名到 ds-success/ds-warning
// 着色规则与主迁移脚本对齐：50/100/200/800/900 → soft;300/400/500/600/700 → DEFAULT
import fs from 'node:fs';

const filePath = 'data/kungfu_data.json';
const original = fs.readFileSync(filePath, 'utf8');

function buildPattern(hue, shades) {
  // 匹配：(bg|text|border|ring)[-:前缀]hue-shade[/opacity]
  // 支持 dark: 前缀；支持 /30 透明度修饰
  const alts = shades.join('|');
  return new RegExp(
    String.raw`\b(bg|text|border|ring)(-(?:[a-z]+:))?(-${hue}-(?:${alts}))((\/\d+)?)\b`,
    'g'
  );
}

function replacer(match, prefix, darkVariant, hueShade, opacity) {
  const isSoft = ['-50', '-100', '-200', '-800', '-900'].some(s => hueShade.endsWith(s));
  const semantic = hueShade.startsWith('-emerald') ? 'success' : 'warning';
  const name = isSoft ? `${semantic}-soft` : semantic;
  return `${prefix}${darkVariant || ''}-ds-${name}${opacity || ''}`;
}

const emeraldSoftRe = buildPattern('emerald', ['50', '100', '200', '800', '900']);
const emeraldDefRe = buildPattern('emerald', ['300', '400', '500', '600', '700']);
const amberSoftRe = buildPattern('amber', ['50', '100', '200', '800', '900']);
const amberDefRe = buildPattern('amber', ['300', '400', '500', '600', '700']);

let patched = original;
patched = patched.replace(emeraldSoftRe, replacer);
patched = patched.replace(emeraldDefRe, replacer);
patched = patched.replace(amberSoftRe, replacer);
patched = patched.replace(amberDefRe, replacer);

if (patched === original) {
  console.log('✗ 没有替换发生,请检查正则');
  process.exit(1);
}

fs.writeFileSync(filePath, patched, 'utf8');

// 统计
const stat = (re, label) => {
  const before = (original.match(re) || []).length;
  const after = (patched.match(re) || []).length;
  console.log(`  ${label}: before=${before} → after=${after}`);
};

console.log(`✓ ${filePath} 已迁移`);
console.log('--- 残留检查 ---');
stat(/(emerald|amber)-[0-9]/g, 'emerald/amber-* 残留');
stat(/ds-(success|warning)/g, 'ds-success/warning 已替换');
stat(/ds-(success|warning)-soft/g, 'soft 变体');
