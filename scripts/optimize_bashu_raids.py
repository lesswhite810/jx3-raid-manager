import re

# Read the file
with open('e:/Data/jx3-raid-manager/data/staticRaids.ts', 'r', encoding='utf-8') as f:
    content = f.read()

# Find the 巴蜀风云 section and replace it
pattern = r'  // 巴蜀风云 \(80级\).*?  // 安史之乱 \(90级\)'

replacement = '''  // 巴蜀风云 (80级)
  // 优化后的副本列表（移除所有挑战模式，添加持国回忆录）
  
  // 龙渊泽：10人普通/英雄，25人普通/英雄
  {
    name: '龙渊泽',
    level: 80,
    version: '巴蜀风云',
    description: '10人普通模式 - 龙渊泽'
  },
  {
    name: '龙渊泽',
    level: 80,
    version: '巴蜀风云',
    description: '10人英雄模式 - 龙渊泽'
  },
  {
    name: '龙渊泽',
    level: 80,
    version: '巴蜀风云',
    description: '25人普通模式 - 龙渊泽',
    isActive: false
  },
  {
    name: '龙渊泽',
    level: 80,
    version: '巴蜀风云',
    description: '25人英雄模式 - 龙渊泽',
    isActive: false
  },
  
  // 荻花圣殿：10人普通/英雄，25人普通/英雄
  {
    name: '荻花圣殿',
    level: 80,
    version: '巴蜀风云',
    description: '10人普通模式 - 80年代开启副本'
  },
  {
    name: '荻花圣殿',
    level: 80,
    version: '巴蜀风云',
    description: '10人英雄模式 - 80年代开启副本'
  },
  {
    name: '荻花圣殿',
    level: 80,
    version: '巴蜀风云',
    description: '25人普通模式 - 80年代开启副本',
    isActive: false
  },
  {
    name: '荻花圣殿',
    level: 80,
    version: '巴蜀风云',
    description: '25人英雄模式 - 80年代开启副本',
    isActive: false
  },
  
  // 烛龙殿：10人普通，25人普通/英雄
  {
    name: '烛龙殿',
    level: 80,
    version: '巴蜀风云',
    description: '10人普通模式 - 烛龙殿'
  },
  {
    name: '烛龙殿',
    level: 80,
    version: '巴蜀风云',
    description: '25人普通模式 - 烛龙殿',
    isActive: false
  },
  {
    name: '烛龙殿',
    level: 80,
    version: '巴蜀风云',
    description: '25人英雄模式 - 烛龙殿',
    isActive: false
  },
  
  // 持国回忆录：10人普通，25人英雄
  {
    name: '持国回忆录',
    level: 80,
    version: '巴蜀风云',
    description: '10人普通模式 - 持国回忆录'
  },
  {
    name: '持国回忆录',
    level: 80,
    version: '巴蜀风云',
    description: '25人英雄模式 - 持国回忆录',
    isActive: false
  },
  
  // 会战唐门：10人普通，25人英雄
  {
    name: '会战唐门',
    level: 80,
    version: '巴蜀风云',
    description: '10人普通模式 - 会战唐门'
  },
  {
    name: '会战唐门',
    level: 80,
    version: '巴蜀风云',
    description: '25人英雄模式 - 会战唐门',
    isActive: false
  },
  
  // 南诏皇宫：10人普通，25人普通/英雄
  {
    name: '南诏皇宫',
    level: 80,
    version: '巴蜀风云',
    description: '10人普通模式 - 80年代结束副本'
  },
  {
    name: '南诏皇宫',
    level: 80,
    version: '巴蜀风云',
    description: '25人普通模式 - 80年代结束副本',
    isActive: false
  },
  {
    name: '南诏皇宫',
    level: 80,
    version: '巴蜀风云',
    description: '25人英雄模式 - 80年代结束副本',
    isActive: false
  },

  // 安史之乱 (90级)'''

# Perform the replacement
new_content = re.sub(pattern, replacement, content, flags=re.DOTALL)

# Write the modified content
with open('e:/Data/jx3-raid-manager/data/staticRaids.ts', 'w', encoding='utf-8') as f:
    f.write(new_content)

print("巴蜀风云副本优化完成！")
