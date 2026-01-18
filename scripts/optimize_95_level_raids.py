import re

# Read the file
with open('e:/Data/jx3-raid-manager/data/staticRaids.ts', 'r', encoding='utf-8') as f:
    content = f.read()

# Optimize all three sections

# 1. Sword Heart (剑胆琴心) section
print("=== 优化剑胆琴心副本 ===")
sword_heart_pattern = r'  // 剑胆琴心 \(95级初期\).*?  // 风骨霸刀 \(95级中期\)'
sword_heart_replacement = '''  // 剑胆琴心 (95级初期)
  // 优化后的副本列表
  
  // 永王行宫·仙侣庭园：10人普通，25人英雄/挑战
  {
    name: '永王行宫·仙侣庭园',
    level: 95,
    version: '剑胆琴心',
    description: '10人普通模式 - 李白所在的副本'
  },
  {
    name: '永王行宫·仙侣庭园',
    level: 95,
    version: '剑胆琴心',
    description: '25人英雄模式 - 掉落95级大橙武材料（醉月玄晶）',
    isActive: false
  },
  {
    name: '永王行宫·仙侣庭园',
    level: 95,
    version: '剑胆琴心',
    description: '25人挑战模式 - 掉落95级大橙武材料（醉月玄晶）',
    isActive: false
  },
  
  // 永王行宫·花月别院：10人普通，25人英雄/挑战
  {
    name: '永王行宫·花月别院',
    level: 95,
    version: '剑胆琴心',
    description: '10人普通模式 - 最终BOSS李白'
  },
  {
    name: '永王行宫·花月别院',
    level: 95,
    version: '剑胆琴心',
    description: '25人英雄模式 - 花月别院',
    isActive: false
  },
  {
    name: '永王行宫·花月别院',
    level: 95,
    version: '剑胆琴心',
    description: '25人挑战模式 - 花月别院',
    isActive: false
  },

  // 风骨霸刀 (95级中期)''' 

# 2. Bone Blade (风骨霸刀) section
print("=== 优化风骨霸刀副本 ===")
bone_blade_pattern = r'  // 风骨霸刀 \(95级中期\).*?  // 重制版 \(95级后期\)'
bone_blade_replacement = '''  // 风骨霸刀 (95级中期)
  // 优化后的副本列表
  
  // 上阳宫·观风殿：10人普通，25人英雄
  {
    name: '上阳宫·观风殿',
    level: 95,
    version: '风骨霸刀',
    description: '10人普通模式 - 史思明登场'
  },
  {
    name: '上阳宫·观风殿',
    level: 95,
    version: '风骨霸刀',
    description: '25人英雄模式 - 双本模式',
    isActive: false
  },
  
  // 上阳宫·双曜亭：10人普通，25人英雄
  {
    name: '上阳宫·双曜亭',
    level: 95,
    version: '风骨霸刀',
    description: '10人普通模式 - 黑齿元佑、哥舒翰'
  },
  {
    name: '上阳宫·双曜亭',
    level: 95,
    version: '风骨霸刀',
    description: '25人英雄模式 - 难度较高',
    isActive: false
  },
  
  // 风雷刀谷·锻刀厅：10人普通，25人英雄
  {
    name: '风雷刀谷·锻刀厅',
    level: 95,
    version: '风骨霸刀',
    description: '10人普通模式 - "全民副本"，难度较低'
  },
  {
    name: '风雷刀谷·锻刀厅',
    level: 95,
    version: '风骨霸刀',
    description: '25人英雄模式 - 柳秀岳、解语',
    isActive: false
  },
  
  // 风雷刀谷·千雷殿：10人普通，25人英雄
  {
    name: '风雷刀谷·千雷殿',
    level: 95,
    version: '风骨霸刀',
    description: '10人普通模式 - 难度极高'
  },
  {
    name: '风雷刀谷·千雷殿',
    level: 95,
    version: '风骨霸刀',
    description: '25人英雄模式 - 最终BOSS柳鸾旗',
    isActive: false
  },

  // 重制版 (95级后期)''' 

# 3. Remake (重制版) section
print("=== 优化重制版副本 ===")
remake_pattern = r'  // 重制版 \(95级后期\).*?  // 世外蓬莱 \(100级\)'
remake_replacement = '''  // 重制版 (95级后期)
  // 优化后的副本列表
  
  // 狼牙堡·战兽山：10人普通，25人英雄
  {
    name: '狼牙堡·战兽山',
    level: 95,
    version: '重制版',
    description: '10人普通模式 - 重制版上线后的首个团本'
  },
  {
    name: '狼牙堡·战兽山',
    level: 95,
    version: '重制版',
    description: '25人英雄模式 - 这里的曲云和报九枫是经典',
    isActive: false
  },
  
  // 狼牙堡·燕然峰：10人普通，25人英雄
  {
    name: '狼牙堡·燕然峰',
    level: 95,
    version: '重制版',
    description: '10人普通模式 - 史思明决战'
  },
  {
    name: '狼牙堡·燕然峰',
    level: 95,
    version: '重制版',
    description: '25人英雄模式 - 难度极高，尤其是石斑机制',
    isActive: false
  },
  
  // 狼牙堡·辉天堑：10人普通，25人英雄
  {
    name: '狼牙堡·辉天堑',
    level: 95,
    version: '重制版',
    description: '10人普通模式 - 95年代收官副本'
  },
  {
    name: '狼牙堡·辉天堑',
    level: 95,
    version: '重制版',
    description: '25人英雄模式 - 凌雪阁主厌夜（月泉淮）首秀',
    isActive: false
  },
  {
    name: '狼神殿',
    level: 95,
    version: '重制版',
    description: '10人普通模式 - 史思明'
  },
  {
    name: '狼神殿',
    level: 95,
    version: '重制版',
    description: '25人英雄模式 - 珍稀坐骑"任驰骋" 史思明同款外观',
    isActive: false
  },

  // 世外蓬莱 (100级)''' 

# Apply all replacements
new_content = content

# 1. Optimize Sword Heart section
new_content = re.sub(sword_heart_pattern, sword_heart_replacement, new_content, flags=re.DOTALL)

# 2. Optimize Bone Blade section
new_content = re.sub(bone_blade_pattern, bone_blade_replacement, new_content, flags=re.DOTALL)

# 3. Optimize Remake section
new_content = re.sub(remake_pattern, remake_replacement, new_content, flags=re.DOTALL)

# Write the modified content
with open('e:/Data/jx3-raid-manager/data/staticRaids.ts', 'w', encoding='utf-8') as f:
    f.write(new_content)

print("=== 副本优化完成！ ===")
