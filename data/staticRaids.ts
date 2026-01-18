export interface StaticRaid {
  name: string;
  level: number;
  version: string;
  description?: string;
  isActive?: boolean;
}

export const STATIC_RAIDS: StaticRaid[] = [
  // 风起稻香 (50-70级)
  // 优化后的副本列表
  // 战宝迦兰：10人普通、25人英雄
  {
    name: '战宝迦兰',
    level: 70,
    version: '风起稻香',
    description: '10人普通模式 - 首个大型团本',
    isActive: false
  },
  {
    name: '战宝迦兰',
    level: 70,
    version: '风起稻香',
    description: '25人英雄模式 - 首个大型团本',
    isActive: false
  },
  // 荻花宫后山：10人普通
  {
    name: '荻花宫后山',
    level: 70,
    version: '风起稻香',
    description: '10人普通模式 - 荻花宫后山',
    isActive: false
  },
  // 宫中神武遗迹：10人普通、25人英雄
  {
    name: '宫中神武遗迹',
    level: 70,
    version: '风起稻香',
    description: '10人普通模式 - 宫中神武遗迹',
    isActive: false
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
    description: '10人普通模式 - 持国天王殿',
    isActive: false
  },

  // 巴蜀风云 (80级)
  // 优化后的副本列表（移除所有挑战模式，添加持国回忆录）
  
  // 龙渊泽：10人普通/英雄，25人普通/英雄
  {
    name: '龙渊泽',
    level: 80,
    version: '巴蜀风云',
    description: '10人普通模式 - 龙渊泽',
    isActive: false
  },
  {
    name: '龙渊泽',
    level: 80,
    version: '巴蜀风云',
    description: '10人英雄模式 - 龙渊泽',
    isActive: false
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
    description: '10人普通模式 - 80年代开启副本',
    isActive: false
  },
  {
    name: '荻花圣殿',
    level: 80,
    version: '巴蜀风云',
    description: '10人英雄模式 - 80年代开启副本',
    isActive: false
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
    description: '10人普通模式 - 烛龙殿',
    isActive: false
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
    description: '10人普通模式 - 持国回忆录',
    isActive: false
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
    description: '10人普通模式 - 会战唐门',
    isActive: false
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
    description: '10人普通模式 - 80年代结束副本',
    isActive: false
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

  // 安史之乱 (90级)
  // 优化后的副本列表
  
  // 战宝军械库：10人普通，25人普通/英雄
  {
    name: '战宝军械库',
    level: 90,
    version: '安史之乱',
    description: '10人普通模式 - 战宝军械库',
    isActive: false
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
    description: '10人普通模式 - 大明宫',
    isActive: false
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
    description: '10人普通模式 - 血战天策',
    isActive: false
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
    description: '10人普通模式 - 风雪稻香村',
    isActive: false
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
    description: '10人普通模式 - 秦皇陵',
    isActive: false
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
    description: '10人普通模式 - 太原之战·夜守孤城',
    isActive: false
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
    description: '10人普通模式 - 太原之战·逐虎驱狼',
    isActive: false
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

  // 剑胆琴心 (95级初期)
  // 优化后的副本列表
  
  // 永王行宫·仙侣庭园：10人普通，25人英雄/挑战，25人普通
  {
    name: '永王行宫·仙侣庭园',
    level: 95,
    version: '剑胆琴心',
    description: '10人普通模式 - 李白所在的副本',
    isActive: false
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
  {
    name: '永王行宫·仙侣庭园',
    level: 95,
    version: '剑胆琴心',
    description: '25人普通模式 - 李白所在的副本',
    isActive: false
  },
  
  // 永王行宫·花月别院：10人普通，25人英雄/挑战，25人普通
  {
    name: '永王行宫·花月别院',
    level: 95,
    version: '剑胆琴心',
    description: '10人普通模式 - 最终BOSS李白',
    isActive: false
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
  {
    name: '永王行宫·花月别院',
    level: 95,
    version: '剑胆琴心',
    description: '25人普通模式 - 最终BOSS李白',
    isActive: false
  },

  // 风骨霸刀 (95级中期)
  // 优化后的副本列表
  
  // 上阳宫·观风殿：10人普通，25人英雄，25人普通
  {
    name: '上阳宫·观风殿',
    level: 95,
    version: '风骨霸刀',
    description: '10人普通模式 - 史思明登场',
    isActive: false
  },
  {
    name: '上阳宫·观风殿',
    level: 95,
    version: '风骨霸刀',
    description: '25人英雄模式 - 双本模式',
    isActive: false
  },
  {
    name: '上阳宫·观风殿',
    level: 95,
    version: '风骨霸刀',
    description: '25人普通模式 - 史思明登场',
    isActive: false
  },
  
  // 上阳宫·双曜亭：10人普通，25人英雄，25人普通
  {
    name: '上阳宫·双曜亭',
    level: 95,
    version: '风骨霸刀',
    description: '10人普通模式 - 黑齿元佑、哥舒翰',
    isActive: false
  },
  {
    name: '上阳宫·双曜亭',
    level: 95,
    version: '风骨霸刀',
    description: '25人英雄模式 - 难度较高',
    isActive: false
  },
  {
    name: '上阳宫·双曜亭',
    level: 95,
    version: '风骨霸刀',
    description: '25人普通模式 - 黑齿元佑、哥舒翰',
    isActive: false
  },
  
  // 风雷刀谷·锻刀厅：10人普通，25人英雄，25人普通
  {
    name: '风雷刀谷·锻刀厅',
    level: 95,
    version: '风骨霸刀',
    description: '10人普通模式 - "全民副本"，难度较低',
    isActive: false
  },
  {
    name: '风雷刀谷·锻刀厅',
    level: 95,
    version: '风骨霸刀',
    description: '25人英雄模式 - 柳秀岳、解语',
    isActive: false
  },
  {
    name: '风雷刀谷·锻刀厅',
    level: 95,
    version: '风骨霸刀',
    description: '25人普通模式 - "全民副本"，难度较低',
    isActive: false
  },
  
  // 风雷刀谷·千雷殿：10人普通，25人英雄，25人普通
  {
    name: '风雷刀谷·千雷殿',
    level: 95,
    version: '风骨霸刀',
    description: '10人普通模式 - 难度极高',
    isActive: false
  },
  {
    name: '风雷刀谷·千雷殿',
    level: 95,
    version: '风骨霸刀',
    description: '25人英雄模式 - 最终BOSS柳鸾旗',
    isActive: false
  },
  {
    name: '风雷刀谷·千雷殿',
    level: 95,
    version: '风骨霸刀',
    description: '25人普通模式 - 难度极高',
    isActive: false
  },

  // 重制版 (95级后期)
  // 优化后的副本列表
  
  // 狼牙堡·战兽山：10人普通，25人英雄，25人普通
  {
    name: '狼牙堡·战兽山',
    level: 95,
    version: '重制版',
    description: '10人普通模式 - 重制版上线后的首个团本',
    isActive: false
  },
  {
    name: '狼牙堡·战兽山',
    level: 95,
    version: '重制版',
    description: '25人英雄模式 - 这里的曲云和报九枫是经典',
    isActive: false
  },
  {
    name: '狼牙堡·战兽山',
    level: 95,
    version: '重制版',
    description: '25人普通模式 - 重制版上线后的首个团本',
    isActive: false
  },
  
  // 狼牙堡·燕然峰：10人普通，25人英雄，25人普通
  {
    name: '狼牙堡·燕然峰',
    level: 95,
    version: '重制版',
    description: '10人普通模式 - 史思明决战',
    isActive: false
  },
  {
    name: '狼牙堡·燕然峰',
    level: 95,
    version: '重制版',
    description: '25人英雄模式 - 难度极高，尤其是石斑机制',
    isActive: false
  },
  {
    name: '狼牙堡·燕然峰',
    level: 95,
    version: '重制版',
    description: '25人普通模式 - 史思明决战',
    isActive: false
  },
  
  // 狼牙堡·辉天堑：10人普通，25人英雄，25人普通
  {
    name: '狼牙堡·辉天堑',
    level: 95,
    version: '重制版',
    description: '10人普通模式 - 95年代收官副本',
    isActive: false
  },
  {
    name: '狼牙堡·辉天堑',
    level: 95,
    version: '重制版',
    description: '25人英雄模式 - 凌雪阁主厌夜（月泉淮）首秀',
    isActive: false
  },
  {
    name: '狼牙堡·辉天堑',
    level: 95,
    version: '重制版',
    description: '25人普通模式 - 95年代收官副本',
    isActive: false
  },
  // 狼神殿：10人普通，25人英雄，25人普通
  {
    name: '狼神殿',
    level: 95,
    version: '重制版',
    description: '10人普通模式 - 史思明',
    isActive: false
  },
  {
    name: '狼神殿',
    level: 95,
    version: '重制版',
    description: '25人英雄模式 - 珍稀坐骑"任驰骋" 史思明同款外观',
    isActive: false
  },
  {
    name: '狼神殿',
    level: 95,
    version: '重制版',
    description: '25人普通模式 - 史思明',
    isActive: false
  },

  // 世外蓬莱 (100级)
  // 25人副本
  {
    name: '荒血路',
    level: 100,
    version: '世外蓬莱',
    description: '25人普通模式 - 冰火岛·荒血路',
    isActive: false,
  },
  {
    name: '荒血路',
    level: 100,
    version: '世外蓬莱',
    description: '25人普通模式 - 冰火岛·荒血路',
    isActive: false,
  },
  {
    name: '荒血路',
    level: 100,
    version: '世外蓬莱',
    description: '25人英雄模式 - 冰火岛·荒血路',
    isActive: false,
  },
  {
    name: '青莲狱',
    level: 100,
    version: '世外蓬莱',
    description: '25人普通模式 - 冰火岛·青莲狱',
    isActive: false,
  },
  {
    name: '青莲狱',
    level: 100,
    version: '世外蓬莱',
    description: '25人普通模式 - 冰火岛·青莲狱',
    isActive: false,
  },
  {
    name: '青莲狱',
    level: 100,
    version: '世外蓬莱',
    description: '25人英雄模式 - 冰火岛·青莲狱',
    isActive: false,
  },
  {
    name: '巨冥湾',
    level: 100,
    version: '世外蓬莱',
    description: '25人普通模式 - 巨冥湾',
    isActive: false,
  },
  {
    name: '巨冥湾',
    level: 100,
    version: '世外蓬莱',
    description: '25人普通模式 - 巨冥湾',
    isActive: false,
  },
  {
    name: '巨冥湾',
    level: 100,
    version: '世外蓬莱',
    description: '25人英雄模式 - 巨冥湾',
    isActive: false,
  },
  {
    name: '饕餮洞',
    level: 100,
    version: '世外蓬莱',
    description: '25人普通模式 - 饕餮洞',
    isActive: false,
  },
  {
    name: '饕餮洞',
    level: 100,
    version: '世外蓬莱',
    description: '25人普通模式 - 饕餮洞',
    isActive: false,
  },
  {
    name: '敖龙岛',
    level: 100,
    version: '世外蓬莱',
    description: '25人普通模式 - 敖龙岛',
    isActive: false,
  },
  {
    name: '敖龙岛',
    level: 100,
    version: '世外蓬莱',
    description: '25人普通模式 - 敖龙岛',
    isActive: false,
  },
  {
    name: '范阳夜变',
    level: 100,
    version: '世外蓬莱',
    description: '25人普通模式 - 范阳夜变',
    isActive: false,
  },
  {
    name: '范阳夜变',
    level: 100,
    version: '世外蓬莱',
    description: '25人普通模式 - 范阳夜变',
    isActive: false,
  },
  // 10人副本
  {
    name: '荒血路',
    level: 100,
    version: '世外蓬莱',
    description: '10人普通模式 - 冰火岛·荒血路'
  },
  {
    name: '青莲狱',
    level: 100,
    version: '世外蓬莱',
    description: '10人普通模式 - 冰火岛·青莲狱'
  },
  {
    name: '巨冥湾',
    level: 100,
    version: '世外蓬莱',
    description: '10人普通模式 - 巨冥湾'
  },
  {
    name: '饕餮洞',
    level: 100,
    version: '世外蓬莱',
    description: '10人普通模式 - 饕餮洞'
  },
  {
    name: '敖龙岛',
    level: 100,
    version: '世外蓬莱',
    description: '10人普通模式 - 敖龙岛'
  },
  {
    name: '范阳夜变',
    level: 100,
    version: '世外蓬莱',
    description: '10人普通模式 - 范阳夜变'
  },
  
  // 奉天证道 (110级)
  // 25人副本
  {
    name: '达摩洞',
    level: 110,
    version: '奉天证道',
    description: '25人普通模式 - 110级副本',
    isActive: false,
  },
  {
    name: '达摩洞',
    level: 110,
    version: '奉天证道',
    description: '25人英雄模式 - 110级副本',
    isActive: false,
  },
  {
    name: '白帝江关',
    level: 110,
    version: '奉天证道',
    description: '25人普通模式 - 110级副本',
    isActive: false,
  },
  {
    name: '白帝江关',
    level: 110,
    version: '奉天证道',
    description: '25人英雄模式 - 110级副本',
    isActive: false,
  },
  {
    name: '雷域大泽',
    level: 110,
    version: '奉天证道',
    description: '25人普通模式 - 110级副本',
    isActive: false,
  },
  {
    name: '雷域大泽',
    level: 110,
    version: '奉天证道',
    description: '25人英雄模式 - 110级副本',
    isActive: false,
  },
  {
    name: '河阳之战',
    level: 110,
    version: '奉天证道',
    description: '25人普通模式 - 110级副本',
    isActive: false,
  },
  {
    name: '河阳之战',
    level: 110,
    version: '奉天证道',
    description: '25人英雄模式 - 110级副本',
    isActive: false,
  },
  // 10人副本
  {
    name: '达摩洞',
    level: 110,
    version: '奉天证道',
    description: '10人普通模式 - 110级副本'
  },
  {
    name: '白帝江关',
    level: 110,
    version: '奉天证道',
    description: '10人普通模式 - 110级副本'
  },
  {
    name: '雷域大泽',
    level: 110,
    version: '奉天证道',
    description: '10人普通模式 - 110级副本'
  },
  {
    name: '河阳之战',
    level: 110,
    version: '奉天证道',
    description: '10人普通模式 - 110级副本'
  },
  
  // 横刀断浪 (120级)
  // 25人副本
  {
    name: '西津渡',
    level: 120,
    version: '横刀断浪',
    description: '25人普通模式 - 120级副本',
    isActive: false,
  },
  {
    name: '西津渡',
    level: 120,
    version: '横刀断浪',
    description: '25人英雄模式 - 120级副本',
    isActive: false,
  },
  {
    name: '武狱黑牢',
    level: 120,
    version: '横刀断浪',
    description: '25人普通模式 - 120级副本',
    isActive: false,
  },
  {
    name: '武狱黑牢',
    level: 120,
    version: '横刀断浪',
    description: '25人英雄模式 - 120级副本',
    isActive: false,
  },
  {
    name: '九老洞',
    level: 120,
    version: '横刀断浪',
    description: '25人普通模式 - 120级副本',
    isActive: false,
  },
  {
    name: '九老洞',
    level: 120,
    version: '横刀断浪',
    description: '25人英雄模式 - 120级副本',
    isActive: false,
  },
  {
    name: '冷龙峰',
    level: 120,
    version: '横刀断浪',
    description: '25人普通模式 - 120级副本',
    isActive: false,
  },
  {
    name: '冷龙峰',
    level: 120,
    version: '横刀断浪',
    description: '25人英雄模式 - 120级副本',
    isActive: false,
  },
  // 10人副本
  {
    name: '西津渡',
    level: 120,
    version: '横刀断浪',
    description: '10人普通模式 - 120级副本'
  },
  {
    name: '武狱黑牢',
    level: 120,
    version: '横刀断浪',
    description: '10人普通模式 - 120级副本'
  },
  {
    name: '九老洞',
    level: 120,
    version: '横刀断浪',
    description: '10人普通模式 - 120级副本'
  },
  {
    name: '冷龙峰',
    level: 120,
    version: '横刀断浪',
    description: '10人普通模式 - 120级副本'
  },

  // 丝路风雨 (130级)
  // 一之窟
  {
    name: '一之窟',
    level: 130,
    version: '丝路风雨',
    description: '10人模式 - 130级副本'
  },
  {
    name: '一之窟',
    level: 130,
    version: '丝路风雨',
    description: '25人普通模式 - 130级副本',
    isActive: false,
  },
  {
    name: '一之窟',
    level: 130,
    version: '丝路风雨',
    description: '25人英雄模式 - 130级副本',
    isActive: false,
  },
  // 太极宫
  {
    name: '太极宫',
    level: 130,
    version: '丝路风雨',
    description: '10人模式 - 130级副本'
  },
  {
    name: '太极宫',
    level: 130,
    version: '丝路风雨',
    description: '25人普通模式 - 130级副本',
    isActive: false,
  },
  {
    name: '太极宫',
    level: 130,
    version: '丝路风雨',
    description: '25人英雄模式 - 130级副本',
    isActive: false,
  },
  // 弓月城
  {
    name: '弓月城',
    level: 130,
    version: '丝路风雨',
    description: '10人模式 - 130级副本'
  },
  {
    name: '弓月城',
    level: 130,
    version: '丝路风雨',
    description: '25人普通模式 - 130级副本'
  },
  {
    name: '弓月城',
    level: 130,
    version: '丝路风雨',
    description: '25人英雄模式 - 130级副本'
  },
  {
    name: '缚罪之渊',
    level: 130,
    version: '丝路风雨',
    description: '25人挑战模式 - 130级挑战副本'
  }
];
