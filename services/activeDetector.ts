import { invoke } from '@tauri-apps/api/core';
import type { BatchActiveResult, Jx3RuntimeStatus } from '../types';

export const activeDetectorService = {
  /**
   * 扫描所有茗伊账号目录并检测活跃状态
   *
   * 注意：不再接收 accountIds 参数，后端会扫描 `interface/my#data/` 下所有
   * `{uid}@zhcn_hd` 目录，解析 info.jx3dat 获取角色身份信息。
   * 调用方通过 roleName + server 匹配数据库角色，聚合到账号级别。
   */
  async detectAccountsActive(gameDirectory: string): Promise<BatchActiveResult> {
    return invoke<BatchActiveResult>('detect_accounts_active', {
      gameDirectory,
    });
  },

  async getJx3RuntimeStatus(gameDirectory: string): Promise<Jx3RuntimeStatus> {
    return invoke<Jx3RuntimeStatus>('get_jx3_runtime_status', {
      gameDirectory,
    });
  },
};
