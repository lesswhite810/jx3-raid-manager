/**
 * 副本类型识别服务
 */

/**
 * 识别副本类型
 * @param dungeonName 副本名称
 * @returns 'raid' 或 'baizhan'
 */
export function identifyDungeonType(dungeonName: string): 'raid' | 'baizhan' {
  // 百战识别
  if (dungeonName.includes('百战')) {
    return 'baizhan';
  }

  // 默认为团队副本
  return 'raid';
}

/**
 * 从百战副本名称提取层数
 * @param dungeonName 副本名称（如 "百战·30层"）
 * @returns 层数，解析失败返回 1
 */
export function extractBaizhanLevel(dungeonName: string): number {
  // 匹配 "百战·30层" 或 "百战30层" 格式
  const match = dungeonName.match(/百战[·•]?(\d+)层?/);
  if (match) {
    return parseInt(match[1], 10);
  }
  return 1;
}

/**
 * 解析团队副本信息
 * @param dungeonName 副本名称（如 "25人英雄冷龙峰"）
 * @returns 解析结果 { playerCount, difficulty, name }
 */
export function parseRaidInfo(dungeonName: string): {
  playerCount: number;
  difficulty: string;
  name: string;
} {
  // 匹配 "25人英雄冷龙峰" 格式
  const match = dungeonName.match(/(\d+)人(普通|英雄|挑战)?(.+)/);
  if (match) {
    return {
      playerCount: parseInt(match[1], 10),
      difficulty: match[2] || '普通',
      name: match[3].trim(),
    };
  }

  // 无法解析，返回默认值
  return {
    playerCount: 25,
    difficulty: '普通',
    name: dungeonName,
  };
}

/**
 * 检查是否是有效的副本名称
 */
export function isValidDungeonName(name: string): boolean {
  if (!name || name.trim().length === 0) return false;

  // 检查是否包含基本的关键词
  const validPatterns = [
    /百战/,                           // 百战
    /\d+人(普通|英雄|挑战).+/,        // 团队副本
  ];

  return validPatterns.some(pattern => pattern.test(name));
}
