import re

# Read the file
with open('e:/Data/jx3-raid-manager/data/staticRaids.ts', 'r', encoding='utf-8') as f:
    content = f.read()

# Find the 风起稻香 section and replace it
pattern = r'  // 风起稻香 \(50-70级\).*?  // 巴蜀风云 \(80级\)'

replacement = '''  // 风起稻香 (50-70级)
  // 优化后的副本列表
  // 战宝迦兰：10人普通、25人英雄
  {
    name: '战宝迦兰',
    level: 70,
    version: '风起稻香',
    description: '10人普通模式 - 首个大型团本'
  },
  {
    name: '战宝迦兰',
    level: 70,
    version: '风起稻香',
    description: '25人英雄模式 - 首个大型团本'
  },
  // 荻花宫后山：10人普通
  {
    name: '荻花宫后山',
    level: 70,
    version: '风起稻香',
    description: '10人普通模式 - 荻花宫后山'
  },
  // 宫中神武遗迹：10人普通、25人英雄
  {
    name: '宫中神武遗迹',
    level: 70,
    version: '风起稻香',
    description: '10人普通模式 - 宫中神武遗迹'
  },
  {
    name: '宫中神武遗迹',
    level: 70,
    version: '风起稻香',
    description: '25人英雄模式 - 宫中神武遗迹',
    isActive: false
  },
  // 持国天王殿：10人普通
  {
    name: '持国天王殿',
    level: 70,
    version: '风起稻香',
    description: '10人普通模式 - 持国天王殿'
  },

  // 巴蜀风云 (80级)'''

# Perform the replacement
new_content = re.sub(pattern, replacement, content, flags=re.DOTALL)

# Write the modified content
with open('e:/Data/jx3-raid-manager/data/staticRaids.ts', 'w', encoding='utf-8') as f:
    f.write(new_content)

print("风起稻香副本优化完成！")
