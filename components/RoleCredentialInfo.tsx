import React, { useEffect, useRef, useState } from 'react';
import { Check, Copy, FileText } from 'lucide-react';

export interface RoleCredentialInfoProps {
  /** 账号名，直接展示并可复制 */
  accountName: string;
  /** 账号密码，缺失时只显示「未配置密码」 */
  password?: string;
  /** 代清账号备注，仅备注存在时渲染 */
  accountNote?: string;
}

/** 密码只显示掩码，真实内容通过复制按钮取用 */
const maskPassword = (password: string): string => '•'.repeat(Math.min(Math.max(password.length, 6), 12));

/**
 * 角色卡片底部的账号 / 备注 / 密码区。
 *
 * 这是旧版（v2.2.8 及以前）团队副本、试炼之地、百战角色卡片上的凭证展示区，
 * 账号与密码都只提供「复制」入口，不在界面上明文展示密码。
 * 新版把这些入口收敛到了 CD 概览表的操作列，因此本组件只在旧版模式渲染。
 */
export const RoleCredentialInfo: React.FC<RoleCredentialInfoProps> = ({
  accountName,
  password,
  accountNote,
}) => {
  const [copiedField, setCopiedField] = useState<'account' | 'password' | null>(null);
  const resetTimerRef = useRef<number | null>(null);

  useEffect(() => () => {
    if (resetTimerRef.current !== null) {
      window.clearTimeout(resetTimerRef.current);
    }
  }, []);

  const copyText = async (text: string, field: 'account' | 'password') => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedField(field);
      if (resetTimerRef.current !== null) {
        window.clearTimeout(resetTimerRef.current);
      }
      resetTimerRef.current = window.setTimeout(() => setCopiedField(null), 1500);
    } catch (error) {
      console.error('复制失败:', error);
    }
  };

  const buttonClass = (field: 'account' | 'password') =>
    `flex-shrink-0 p-1 rounded transition-colors ${copiedField === field
      ? 'text-ds-success'
      : 'text-muted hover:text-main hover:bg-surface'}`;

  return (
    <div className="mt-3 pt-3 border-t border-base space-y-2">
      <div className="flex items-center gap-2">
        <div className="text-xs text-muted flex-shrink-0">账号</div>
        <div className="flex items-center gap-1 flex-1 min-w-0 bg-base rounded px-2 py-1">
          <span className="text-xs text-main truncate flex-1">{accountName}</span>
          <button
            onClick={(event) => {
              event.stopPropagation();
              void copyText(accountName, 'account');
            }}
            className={buttonClass('account')}
            title={copiedField === 'account' ? '已复制!' : '复制账号'}
            aria-label="复制账号"
          >
            {copiedField === 'account' ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
          </button>
        </div>
      </div>

      {accountNote && (
        <div className="flex items-start gap-2">
          <div className="text-xs text-muted flex-shrink-0 pt-1">备注</div>
          <div className="flex items-start gap-1.5 flex-1 min-w-0 bg-ds-success-soft border border-ds-success-soft rounded px-2 py-1.5">
            <FileText className="w-3.5 h-3.5 text-ds-success-strong flex-shrink-0 mt-0.5" />
            <span className="text-xs text-ds-success-strong-soft break-all">{accountNote}</span>
          </div>
        </div>
      )}

      <div className="flex items-center gap-2">
        <div className="text-xs text-muted flex-shrink-0">密码</div>
        {password ? (
          <div className="flex items-center gap-1 flex-1 min-w-0 bg-base rounded px-2 py-1">
            <span className="text-xs text-main font-mono truncate flex-1">{maskPassword(password)}</span>
            <button
              onClick={(event) => {
                event.stopPropagation();
                void copyText(password, 'password');
              }}
              className={buttonClass('password')}
              title={copiedField === 'password' ? '已复制!' : '复制密码'}
              aria-label="复制密码"
            >
              {copiedField === 'password' ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
            </button>
          </div>
        ) : (
          <div className="flex-1 min-w-0 bg-base rounded px-2 py-1">
            <span className="text-xs text-muted truncate flex-1">未配置密码</span>
          </div>
        )}
      </div>
    </div>
  );
};
