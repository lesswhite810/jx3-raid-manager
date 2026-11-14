import { invokeCommand } from './invoke';
import { RaidRecord } from '../types';

/**
 * 进行中的试炼（与后端 TrialInProgress 对齐）
 */
export interface TrialInProgress {
  /** 试炼地图 ID */
  mapId: number;
  /** 当前层数 */
  layer?: number;
  /** 已通过的第几关（1 或 2） */
  stage: number;
  /** 第一、二关已知的首领名，未过的关卡为空字符串 */
  bosses: [string, string];
  /** 最后一次试炼事件的毫秒时间戳 */
  lastEventAtMs: number;
  /** 是否存在跨时段续打存档 */
  savedProgress: boolean;
}

/**
 * 单个账号一次扫描的总结果（与后端 RaidScanOutcome 对齐）
 */
export interface RaidScanOutcome {
  /** 写入/更新的记录总数（常规副本实例 + 试炼通关记录） */
  insertedCount: number;
  /** 正在打但尚未出终局的试炼（无则为空） */
  trialInProgress?: TrialInProgress;
}

/**
 * 单个账号的批量扫描结果（与后端 AccountScanResult 对齐）
 */
export interface AccountScanResult {
  /** 茗伊账号 UID */
  accountId: string;
  /** 是否扫描成功 */
  success: boolean;
  /** 扫描到的副本实例数量（成功时） */
  instanceCount?: number;
  /** 已有通关的翻牌明细发生变化，通关数可以为零。 */
  trialDrawsChanged?: boolean;
  /** 正在打但尚未出终局的试炼（无则为空） */
  trialInProgress?: TrialInProgress;
  /** 失败原因（失败时） */
  error?: string;
}

/**
 * 带账号归属的进行中试炼（供 UI 展示）
 */
export interface TrialInProgressWithAccount extends TrialInProgress {
  /** 茗伊账号 UID */
  accountId: string;
}

/**
 * 副本掉落自动扫描服务（B 阶段）
 *
 * 包装 Tauri 后端 scan_raid_drops / scan_all_active_raid_drops / confirm_record / reject_record 命令。
 * 详见 src-tauri/src/mingyi/drop_scanner.rs
 */
export const dropScannerService = {
  /**
   * 扫描指定账号（茗伊 uid）的 JCL 与 chatlog，
   * 将识别到的副本掉落记录以 status='pending' 写入 records 表。
   *
   * @param accountId 茗伊 uid（{uid}@zhcn_hd 目录名中的数字部分）
   * @returns 本次扫描的总结果（写入数量 + 进行中试炼）
   */
  async scanRaidDrops(accountId: string): Promise<RaidScanOutcome> {
    return invokeCommand<RaidScanOutcome>('scan_raid_drops', { accountId });
  },

  /**
   * 批量扫描所有活跃账号（combat_logs 目录 mtime 晚于 JX3 进程启动时间）的掉落记录。
   * 内部对每个活跃账号调用 scan_raid_drops 逻辑。
   *
   * @returns 每个活跃账号的扫描结果列表
   */
  async scanAllActiveRaidDrops(): Promise<AccountScanResult[]> {
    return invokeCommand<AccountScanResult[]>('scan_all_active_raid_drops');
  },

  /**
   * 按时间范围扫描所有账号的掉落记录（离线扫描，不依赖 JX3 进程状态）。
   *
   * 用于"扫描本周"等离线场景：遍历数据库中所有账号，
   * 按副本开始时间（JCL 文件名解析）过滤，仅扫描 [startMs, endMs) 范围内的副本。
   * 与 scanAllActiveRaidDrops 不同，不依赖 JX3 进程运行状态，扫描所有账号。
   *
   * @param startMs 副本时间下限（毫秒，闭区间）
   * @param endMs 副本时间上限（毫秒，开区间）
   * @param processStartMs JX3 进程启动时间（毫秒），> 0 时按 mtime 过滤 JCL 文件
   * @returns 每个账号的扫描结果列表
   */
  async scanRaidsInRange(
    startMs: number,
    endMs: number,
    processStartMs: number = 0,
  ): Promise<AccountScanResult[]> {
    return invokeCommand<AccountScanResult[]>('scan_raids_in_range', {
      startMs,
      endMs,
      processStartMs,
    });
  },

  /**
   * 确认一条 pending 记录，将其 status 更新为 'confirmed'。
   * confirmed 记录与手动记录完全等价参与 CD 计算。
   *
   * @param editData 可选的编辑数据（RaidRecord 部分字段），提供时会合并到 data JSON
   */
  async confirmRecord(recordId: string, editData?: Partial<RaidRecord>): Promise<void> {
    const editDataStr = editData ? JSON.stringify(editData) : undefined;
    await invokeCommand('confirm_record', { recordId, editData: editDataStr });
  },

  /**
   * 拒绝一条 pending 记录，将其 status 更新为 'rejected'。
   * rejected 记录不参与 CD 计算，等价于该副本实例未发生。
   */
  async rejectRecord(recordId: string): Promise<void> {
    await invokeCommand('reject_record', { recordId });
  },

  /**
   * 清空 JCL 解析缓存表（jcl_cache）。
   *
   * 当 analyze_jcl 逻辑修复后，旧版缓存可能残留错误的 is_kill 判定，
   * 调用此方法清空缓存，下次扫描时所有 JCL 文件会重新解析。
   *
   * @returns 被清除的缓存条目数量
   */
  async clearJclCache(): Promise<number> {
    return invokeCommand<number>('clear_jcl_cache');
  },
};
