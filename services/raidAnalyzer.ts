/**
 * 自动分析服务
 * 调用 Rust 后端的 analyzer_analyze 接口进行一键分析
 */

import { invoke } from '@tauri-apps/api/core';
import {
  TimeRangeType,
  TimeRange,
  AnalysisResult,
  AnalysisProgress,
  SpecialItem,
  RaidRecord,
  BaizhanRecord,
  AnalyzeResponse,
  RoleAnalyzeResultRaw,
  AnalyzeRecordRaw,
  DropFlags,
} from '../types';
import { getTimeRangeBounds } from '../utils/timeRange';
import { identifyDungeonType } from './dungeonType';
import { analyzerLogger, LOG_MODULES } from './analyzerLogger';
import { generateUUID } from '../utils/uuid';

const MODULE = LOG_MODULES.ANALYZER;

/** 分析器配置 */
export interface AnalyzerConfig {
  gameDir: string;
  timeRange: TimeRangeType;
  selectedRoleIds: Set<string>;
}

/** 分析统计 */
export interface AnalysisStats {
  totalResults: number;
  raidCount: number;
  baizhanCount: number;
  duplicateCount: number;
  skippedByTime: number;
  duration: number;
}

/**
 * 分析器类
 */
export class RaidAnalyzer {
  private config: AnalyzerConfig;
  private timeRangeBounds: TimeRange;
  private aborted = false;

  constructor(config: AnalyzerConfig) {
    this.config = config;
    this.timeRangeBounds = getTimeRangeBounds(config.timeRange);
  }

  /**
   * 中止分析
   */
  abort() {
    this.aborted = true;
    analyzerLogger.info(MODULE, '分析已中止');
  }

  /**
   * 执行分析（调用后端统一接口）
   */
  async analyze(
    onProgress: (progress: AnalysisProgress) => void
  ): Promise<{ results: AnalysisResult[]; stats: AnalysisStats }> {
    const startTime = Date.now();
    this.aborted = false;

    analyzerLogger.info(MODULE, '开始分析', {
      gameDir: this.config.gameDir,
      timeRange: this.config.timeRange,
      selectedRoleIds: Array.from(this.config.selectedRoleIds),
    });

    const stats: AnalysisStats = {
      totalResults: 0,
      raidCount: 0,
      baizhanCount: 0,
      duplicateCount: 0,
      skippedByTime: 0,
      duration: 0,
    };

    try {
      // 调用后端统一分析接口
      onProgress({ percent: 20, currentFile: '扫描文件...' });

      if (this.aborted) {
        throw new Error('分析已中止');
      }

      onProgress({ percent: 40, currentFile: '分析 GKP 和聊天记录...' });

      const response = await invoke<AnalyzeResponse>('analyzer_analyze', {
        gameDir: this.config.gameDir,
        timeRange: this.timeRangeBounds,
        selectedRoles: Array.from(this.config.selectedRoleIds),
      });

      if (this.aborted) {
        throw new Error('分析已中止');
      }

      onProgress({ percent: 70, currentFile: '处理分析结果...' });

      // 转换响应为前端格式
      const results = this.transformResponse(response);

      // 统计
      stats.totalResults = results.length;
      stats.raidCount = results.filter(r => r.dungeonType === 'raid').length;
      stats.baizhanCount = results.filter(r => r.dungeonType === 'baizhan').length;
      stats.duration = Date.now() - startTime;

      onProgress({ percent: 100, currentFile: '分析完成' });

      analyzerLogger.info(MODULE, '分析完成', stats);

      return { results, stats };
    } catch (error) {
      analyzerLogger.error(MODULE, '分析失败', {
        error: error instanceof Error ? error.message : String(error),
        stack: error instanceof Error ? error.stack : undefined,
      });
      throw error;
    }
  }

  /**
   * 将后端响应转换为前端 AnalysisResult 格式
   */
  private transformResponse(response: AnalyzeResponse): AnalysisResult[] {
    const results: AnalysisResult[] = [];

    for (const roleResult of response.roles) {
      for (const record of roleResult.records) {
        const result = this.transformRecord(roleResult, record);
        results.push(result);
      }
    }

    return results;
  }

  /**
   * 转换单条记录
   */
  private transformRecord(role: RoleAnalyzeResultRaw, record: AnalyzeRecordRaw): AnalysisResult {
    const { income } = record;

    // 转换 DropFlags（后端使用 snake_case，前端使用 camelCase）
    const dropFlags: DropFlags = {
      hasXuanjing: income.drop_flags.has_xuanjing,
      hasMaJu: income.drop_flags.has_maju,
      hasPet: income.drop_flags.has_pet,
      hasPendant: income.drop_flags.has_pendant,
      hasMount: income.drop_flags.has_mount,
      hasAppearance: income.drop_flags.has_appearance,
      hasTitle: income.drop_flags.has_title,
      hasSecretBook: income.drop_flags.has_secret_book,
    };

    // 转换 SpecialItem
    const specialItems: SpecialItem[] = income.special_items.map(item => ({
      name: item.name,
      buyer: item.buyer,
      price: item.price,
      isWorkerBought: item.is_worker_bought,
    }));

    // 构建备注
    const noteParts: string[] = [];
    if (record.difficulty && record.difficulty !== '普通') {
      noteParts.push(record.difficulty);
    }
    if (record.notes) {
      noteParts.push(record.notes);
    }

    // 判断副本类型
    const dungeonType = identifyDungeonType(record.dungeon_name);

    // 生成 UID
    const uid = this.generateUID({
      startTime: record.start_time,
      endTime: record.end_time,
      dungeonName: record.dungeon_name,
      leaderName: record.leader_name,
      workerName: role.role_name,
      personalIncome: income.income,
      teamIncome: 0,
      scatteredConsumption: income.expense_detail.scattered,
      ironConsumption: income.expense_detail.iron,
      specialConsumption: income.expense_detail.special,
      otherConsumption: income.expense_detail.other,
      fine: 0,
      notes: noteParts.join(' | '),
    });

    const result: AnalysisResult = {
      uid,
      startTime: record.start_time,
      endTime: record.end_time,
      dungeonName: record.dungeon_name,
      dungeonType,
      playerCount: record.player_count,
      difficulty: record.difficulty,
      leaderName: record.leader_name,
      workerName: role.role_name,
      workerAccountId: '',
      workerRoleId: role.role_id,
      workerServer: '',
      personalIncome: income.income,
      teamIncome: 0,
      scatteredConsumption: income.expense_detail.scattered,
      ironConsumption: income.expense_detail.iron,
      specialConsumption: income.expense_detail.special,
      otherConsumption: income.expense_detail.other,
      totalConsumption: income.expense,
      fine: 0,
      subsidy: 0,
      netIncome: income.net_income,
      specialItems,
      dropFlags,
      notes: noteParts.join(' | '),
      isFilled: false,
      sourceDbFile: '',
      sourceGkpFile: '',
    };

    return result;
  }

  /**
   * 生成唯一 UID
   * 拼接所有关键字段后取 hash 前 8 位
   */
  private generateUID(segment: ParsedSegment): string {
    const parts = [
      segment.startTime,
      segment.endTime,
      segment.dungeonName,
      segment.leaderName,
      segment.workerName,
      segment.personalIncome,
      segment.teamIncome ?? 0,
      segment.scatteredConsumption,
      segment.ironConsumption,
      segment.specialConsumption,
      segment.otherConsumption,
      segment.fine,
      segment.notes,
    ];
    const str = parts.join('-');
    // FNV-1a hash（快速且分布均匀）
    let hash = 0x811c9dc5;
    for (let i = 0; i < str.length; i++) {
      hash ^= str.charCodeAt(i);
      hash = Math.imul(hash, 0x01000193);
    }
    return (hash >>> 0).toString(16).padStart(8, '0').slice(0, 8);
  }
}

/** 解析出的段数据（用于生成 UID） */
interface ParsedSegment {
  startTime: number;
  endTime: number;
  dungeonName: string;
  leaderName: string;
  workerName: string;
  personalIncome: number;
  teamIncome?: number;
  scatteredConsumption: number;
  ironConsumption: number;
  specialConsumption: number;
  otherConsumption: number;
  fine: number;
  notes: string;
}

/**
 * 将分析结果映射为团队副本记录
 */
export function mapToRaidRecord(result: AnalysisResult): RaidRecord {
  return {
    id: generateUUID(),
    accountId: result.workerAccountId,
    roleId: result.workerRoleId,
    raidName: result.dungeonName,
    date: result.startTime,
    goldIncome: result.netIncome > 0 ? result.netIncome : 0,
    goldExpense: result.netIncome < 0 ? Math.abs(result.netIncome) : undefined,
    hasXuanjing: result.dropFlags.hasXuanjing,
    hasMaJu: result.dropFlags.hasMaJu,
    hasPet: result.dropFlags.hasPet,
    hasPendant: result.dropFlags.hasPendant,
    hasMount: result.dropFlags.hasMount,
    hasAppearance: result.dropFlags.hasAppearance,
    hasTitle: result.dropFlags.hasTitle,
    hasSecretBook: result.dropFlags.hasSecretBook,
    notes: result.notes || undefined,
    roleName: result.workerName,
    server: result.workerServer,
    transactionType: result.netIncome >= 0 ? 'income' : 'expense',
  };
}

/**
 * 将分析结果映射为百战记录
 */
export function mapToBaizhanRecord(result: AnalysisResult): BaizhanRecord {
  return {
    id: generateUUID(),
    accountId: result.workerAccountId,
    roleId: result.workerRoleId,
    roleName: result.workerName,
    server: result.workerServer,
    date: result.startTime,
    goldIncome: result.netIncome > 0 ? result.netIncome : 0,
    goldExpense: result.netIncome < 0 ? Math.abs(result.netIncome) : undefined,
    notes: result.notes || undefined,
    type: 'baizhan',
  };
}
