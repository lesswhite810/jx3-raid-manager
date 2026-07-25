import { invoke } from '@tauri-apps/api/core';
import { RaidRecord } from '../types';

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
  /** 失败原因（失败时） */
  error?: string;
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
   * @returns 本次扫描处理（新增或更新）的副本实例数量
   */
  async scanRaidDrops(accountId: string): Promise<number> {
    return invoke<number>('scan_raid_drops', { accountId });
  },

  /**
   * 批量扫描所有活跃账号（combat_logs 目录 mtime 晚于 JX3 进程启动时间）的掉落记录。
   * 内部对每个活跃账号调用 scan_raid_drops 逻辑。
   *
   * @returns 每个活跃账号的扫描结果列表
   */
  async scanAllActiveRaidDrops(): Promise<AccountScanResult[]> {
    return invoke<AccountScanResult[]>('scan_all_active_raid_drops');
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
   * @returns 每个账号的扫描结果列表
   */
  async scanRaidsInRange(startMs: number, endMs: number): Promise<AccountScanResult[]> {
    return invoke<AccountScanResult[]>('scan_raids_in_range', { startMs, endMs });
  },

  /**
   * 确认一条 pending 记录，将其 status 更新为 'confirmed'。
   * confirmed 记录与手动记录完全等价参与 CD 计算。
   *
   * @param editData 可选的编辑数据（RaidRecord 部分字段），提供时会合并到 data JSON
   */
  async confirmRecord(recordId: string, editData?: Partial<RaidRecord>): Promise<void> {
    const editDataStr = editData ? JSON.stringify(editData) : undefined;
    await invoke('confirm_record', { recordId, editData: editDataStr });
  },

  /**
   * 拒绝一条 pending 记录，将其 status 更新为 'rejected'。
   * rejected 记录不参与 CD 计算，等价于该副本实例未发生。
   */
  async rejectRecord(recordId: string): Promise<void> {
    await invoke('reject_record', { recordId });
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
    return invoke<number>('clear_jcl_cache');
  },
};
