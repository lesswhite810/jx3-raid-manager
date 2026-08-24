// 给 user-guide.md 与 新手指南.md 各自加上角色定位说明
import fs from 'node:fs';

function detectEOL(s) {
  return s.includes('\r\n') ? '\r\n' : '\n';
}

function addRole(docPath, anchorLines, replacementLines) {
  const s = fs.readFileSync(docPath, 'utf8');
  const EOL = detectEOL(s);
  const anchor = anchorLines.join(EOL);
  const replacement = replacementLines.join(EOL);
  if (s.startsWith(anchor)) {
    const newContent = replacement + s.substring(anchor.length);
    fs.writeFileSync(docPath, newContent, 'utf8');
    console.log('✓ ' + docPath + ' (EOL=' + (EOL === '\r\n' ? 'CRLF' : 'LF') + ')');
  } else {
    console.log('✗ ' + docPath + ' - 锚点未匹配文件首段');
    // Debug
    const sample = s.split('\n').slice(0, 4).join('\\n');
    console.log('  实际首 4 行: ' + JSON.stringify(sample));
  }
}

// === user-guide.md ===
addRole(
  'docs/user-guide.md',
  [
    '# 剑网三副本管家 使用手册',
    '',
    'JX3 Raid Manager 是一款专为《剑网三》玩家打造的本地化副本数据管理工具，帮助你记录副本收入、管理账号角色，并支持自动扫描游戏数据。',
  ],
  [
    '# 剑网三副本管家 使用手册',
    '',
    '> 📚 **完整参考手册** · 何时查这里：想了解某个功能的详细说明、配置项、快捷键、高级玩法',
    '> 💡 配套入门：[`docs/新手指南.md`](./新手指南.md) 是 **step-by-step 入门版**（10 分钟跑起来） · 看一遍就能上手',
    '> 📝 本文档于 2026-08-23 加上角色定位说明，原内容不变',
    '',
    'JX3 Raid Manager 是一款专为《剑网三》玩家打造的本地化副本数据管理工具，帮助你记录副本收入、管理账号角色，并支持自动扫描游戏数据。',
  ]
);

// === 新手指南.md ===
addRole(
  'docs/新手指南.md',
  [
    '# 新手指南',
    '',
    '欢迎使用剑网三副本管家！本指南将帮助你用最简单的方式开始使用这款工具。',
  ],
  [
    '# 新手指南',
    '',
    '> 🎯 **step-by-step 入门** · 何时看这里：第一次使用 · 想 10 分钟内跑起来',
    '> 💡 配套参考：[`docs/user-guide.md`](./user-guide.md) 是 **完整参考手册** · 想了解某个功能的详细配置时再来查',
    '> 📝 本文档于 2026-08-23 加上角色定位说明，原内容不变',
    '',
    '欢迎使用剑网三副本管家！本指南将帮助你用最简单的方式开始使用这款工具。',
  ]
);
