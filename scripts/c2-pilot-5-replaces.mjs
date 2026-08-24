// C2 试点:替换 IncomeDetail.tsx 5 处类名
//   ① L651 text-xs 支出标签  → text-ds-warning-strong
//   ② L671 text-xs 收入标签  → text-ds-success-strong
//   ③ L760 text-sm 收入指示条 → text-ds-success-strong
//   ④ L593 text-sm 收入过滤标签 → text-ds-success-strong
//   ⑤ L595 text-sm 支出过滤标签 → text-ds-warning-strong
import fs from 'node:fs';

const p = 'components/IncomeDetail.tsx';
let s = fs.readFileSync(p, 'utf8');
const EOL = s.includes('\r\n') ? '\r\n' : '\n';

// 5 处精确替换(基于 line context 区分,避免误伤)
const replacements = [
  {
    // L651: text-xs 支出标签
    desc: 'L651 text-xs 支出标签',
    from: 'px-1.5 py-0.5 bg-ds-warning-soft dark:bg-ds-warning-soft/20 text-ds-warning dark:text-ds-warning text-xs font-medium',
    to:   'px-1.5 py-0.5 bg-ds-warning-soft dark:bg-ds-warning-soft/20 text-ds-warning-strong dark:text-ds-warning-strong text-xs font-medium',
  },
  {
    // L671: text-xs 收入标签
    desc: 'L671 text-xs 收入标签',
    from: 'px-1.5 py-0.5 bg-ds-success-soft dark:bg-ds-success-soft/20 text-ds-success dark:text-ds-success text-xs font-medium',
    to:   'px-1.5 py-0.5 bg-ds-success-soft dark:bg-ds-success-soft/20 text-ds-success-strong dark:text-ds-success-strong text-xs font-medium',
  },
  {
    // L760: text-sm 收入指示条
    desc: 'L760 text-sm 收入指示条',
    from: 'flex items-center gap-2 text-ds-success dark:text-ds-success bg-ds-success-soft dark:bg-ds-success-soft/10',
    to:   'flex items-center gap-2 text-ds-success-strong dark:text-ds-success-strong bg-ds-success-soft dark:bg-ds-success-soft/10',
  },
  {
    // L593/595: text-sm summaryColorClass 里的收入 / 支出分支
    desc: 'L593-595 text-sm 收入/支出过滤标签',
    // activeTab === 'income'  → text-ds-success-strong
    from: "                  ? 'text-ds-success dark:text-ds-success'\n                  : activeTab === 'expense'\n                    ? 'text-ds-warning dark:text-ds-warning'",
    to:   "                  ? 'text-ds-success-strong dark:text-ds-success-strong'\n                  : activeTab === 'expense'\n                    ? 'text-ds-warning-strong dark:text-ds-warning-strong'",
  },
  {
    // L597/599: netIncome > 0 / < 0 的派生分支(同样的过滤标签)
    desc: 'L597-599 text-sm 净收入过滤标签',
    from: "                      ? 'text-ds-success dark:text-ds-success'\n                      : netIncome < 0\n                        ? 'text-ds-warning dark:text-ds-warning'",
    to:   "                      ? 'text-ds-success-strong dark:text-ds-success-strong'\n                      : netIncome < 0\n                        ? 'text-ds-warning-strong dark:text-ds-warning-strong'",
  },
];

let okCount = 0;
let failCount = 0;
for (const r of replacements) {
  const fromWithEOL = r.from.split('\n').join(EOL);
  const toWithEOL = r.to.split('\n').join(EOL);
  if (s.includes(fromWithEOL)) {
    if (r.onlyFirst) {
      // 只替换第一个匹配
      const idx = s.indexOf(fromWithEOL);
      s = s.substring(0, idx) + toWithEOL + s.substring(idx + fromWithEOL.length);
    } else {
      s = s.replace(fromWithEOL, toWithEOL);
    }
    console.log('  ✓ ' + r.desc);
    okCount++;
  } else {
    console.log('  ✗ ' + r.desc + ' (未匹配)');
    failCount++;
  }
}

fs.writeFileSync(p, s, 'utf8');
console.log('');
console.log('合计: ' + okCount + ' 处成功 / ' + failCount + ' 处失败');
if (failCount > 0) {
  console.log('(失败的可能已被先前替换或 line 已偏移)');
}