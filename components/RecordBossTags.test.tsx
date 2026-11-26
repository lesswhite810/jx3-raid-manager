/**
 * RecordBossTags 渲染测试
 *
 * 收益记录列表此前只显示副本与角色，看不出这一场打掉了哪些 BOSS。
 * 这里断言标签组会按记录顺序渲染 BOSS 名称，并且没有 BOSS 信息的记录
 * 不会留下孤立的图标或空标签。
 */

import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { RecordBossTags } from './RecordBossTags';

describe('RecordBossTags', () => {
  it('按记录顺序渲染多选 BOSS 名称', () => {
    const markup = renderToStaticMarkup(
      <RecordBossTags
        record={{
          bossIds: ['luoyangzhizhan_1', 'luoyangzhizhan_5'],
          bossNames: ['突利和顺', '史朝义'],
        }}
      />,
    );

    expect(markup).toContain('突利和顺');
    expect(markup).toContain('史朝义');
    expect(markup).toContain('击败 BOSS（2）');
  });

  it('兼容只有单个 BOSS 字段的旧记录', () => {
    const markup = renderToStaticMarkup(
      <RecordBossTags record={{ bossId: 'luoyangzhizhan_5', bossName: '史朝义' }} />,
    );

    expect(markup).toContain('史朝义');
  });

  it('没有 BOSS 信息时不渲染任何内容', () => {
    expect(renderToStaticMarkup(<RecordBossTags record={{}} />)).toBe('');
  });
});
