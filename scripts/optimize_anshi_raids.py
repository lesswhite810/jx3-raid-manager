import re

# Read the file
with open('e:/Data/jx3-raid-manager/data/staticRaids.ts', 'r', encoding='utf-8') as f:
    content = f.read()

# Find the 安史之乱 section and replace it
pattern = r'  // 安史之乱 \(90级\).*?  // 剑胆琴心 \(95级初期\)'

replacement = '''  // 安史之乱 (90级)
  // 优化后的副本列表
  
  // 战宝军械库：10人普通，25人普通/英雄
  {
    name: '战宝军械库',
    level: 90,
    version: '安史之乱',
    description: '10人普通模式 - 战宝军械库'
  },
  {
    name: '战宝军械库',
    level: 90,
    version: '安史之乱',
    description: '25人普通模式 - 战宝军械库',
    isActive: false
  },
  {
    name: '战宝军械库',
    level: 90,
    version: '安史之乱',
    description: '25人英雄模式 - 战宝军械库',
    isActive: false
  },
  
  // 大明宫：10人普通，25人普通/英雄
  {
    name: '大明宫',
    level: 90,
    version: '安史之乱',
    description: '10人普通模式 - 大明宫'
  },
  {
    name: '大明宫',
    level: 90,
    version: '安史之乱',
    description: '25人普通模式 - 大明宫',
    isActive: false
  },
  {
    name: '大明宫',
    level: 90,
    version: '安史之乱',
    description: '25人英雄模式 - 大明宫',
    isActive: false
  },
  
  // 血战天策：10人普通，25人普通/英雄
  {
    name: '血战天策',
    level: 90,
    version: '安史之乱',
    description: '10人普通模式 - 血战天策'
  },
  {
    name: '血战天策',
    level: 90,
    version: '安史之乱',
    description: '25人普通模式 - 血战天策',
    isActive: false
  },
  {
    name: '血战天策',
    level: 90,
    version: '安史之乱',
    description: '25人英雄模式 - 血战天策',
    isActive: false
  },
  
  // 风雪稻香村：10人普通，25人普通/英雄
  {
    name: '风雪稻香村',
    level: 90,
    version: '安史之乱',
    description: '10人普通模式 - 风雪稻香村'
  },
  {
    name: '风雪稻香村',
    level: 90,
    version: '安史之乱',
    description: '25人普通模式 - 风雪稻香村',
    isActive: false
  },
  {
    name: '风雪稻香村',
    level: 90,
    version: '安史之乱',
    description: '25人英雄模式 - 风雪稻香村',
    isActive: false
  },
  
  // 秦皇陵：10人普通，25人普通/英雄
  {
    name: '秦皇陵',
    level: 90,
    version: '安史之乱',
    description: '10人普通模式 - 秦皇陵'
  },
  {
    name: '秦皇陵',
    level: 90,
    version: '安史之乱',
    description: '25人普通模式 - 秦皇陵',
    isActive: false
  },
  {
    name: '秦皇陵',
    level: 90,
    version: '安史之乱',
    description: '25人英雄模式 - 秦皇陵',
    isActive: false
  },
  
  // 夜守孤城：10人普通，25人普通/英雄/挑战
  {
    name: '夜守孤城',
    level: 90,
    version: '安史之乱',
    description: '10人普通模式 - 太原之战·夜守孤城'
  },
  {
    name: '夜守孤城',
    level: 90,
    version: '安史之乱',
    description: '25人普通模式 - 太原之战·夜守孤城',
    isActive: false
  },
  {
    name: '夜守孤城',
    level: 90,
    version: '安史之乱',
    description: '25人英雄模式 - 太原之战·夜守孤城',
    isActive: false
  },
  {
    name: '夜守孤城',
    level: 90,
    version: '安史之乱',
    description: '25人挑战模式 - 太原之战·夜守孤城',
    isActive: false
  },
  
  // 逐虎驱狼：10人普通，25人普通/英雄/挑战
  {
    name: '逐虎驱狼',
    level: 90,
    version: '安史之乱',
    description: '10人普通模式 - 太原之战·逐虎驱狼'
  },
  {
    name: '逐虎驱狼',
    level: 90,
    version: '安史之乱',
    description: '25人普通模式 - 太原之战·逐虎驱狼',
    isActive: false
  },
  {
    name: '逐虎驱狼',
    level: 90,
    version: '安史之乱',
    description: '25人英雄模式 - 太原之战·逐虎驱狼',
    isActive: false
  },
  {
    name: '逐虎驱狼',
    level: 90,
    version: '安史之乱',
    description: '25人挑战模式 - 太原之战·逐虎驱狼',
    isActive: false
  },

  // 剑胆琴心 (95级初期)'''

# Perform the replacement
new_content = re.sub(pattern, replacement, content, flags=re.DOTALL)

# Write the modified content
with open('e:/Data/jx3-raid-manager/data/staticRaids.ts', 'w', encoding='utf-8') as f:
    f.write(new_content)

print("安史之乱副本优化完成！")
