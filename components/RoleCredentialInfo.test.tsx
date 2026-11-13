import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { RoleCredentialInfo } from './RoleCredentialInfo';

const render = (props: Parameters<typeof RoleCredentialInfo>[0]) =>
  renderToStaticMarkup(<RoleCredentialInfo {...props} />);

describe('RoleCredentialInfo - 账号与密码展示', () => {
  it('展示账号名并提供复制账号按钮', () => {
    const markup = render({ accountName: 'werhua520' });

    expect(markup).toContain('账号');
    expect(markup).toContain('werhua520');
    expect(markup).toContain('复制账号');
  });

  it('有密码时展示掩码与复制密码按钮，绝不输出明文', () => {
    const markup = render({ accountName: 'werhua520', password: 'secret123' });

    expect(markup).toContain('密码');
    expect(markup).toContain('复制密码');
    expect(markup).toContain('••••••');
    expect(markup).not.toContain('secret123');
  });

  it('没有密码时提示未配置，且不渲染复制密码按钮', () => {
    const markup = render({ accountName: 'werhua520' });

    expect(markup).toContain('未配置密码');
    expect(markup).not.toContain('复制密码');
  });

  it('有代清备注时一并展示', () => {
    const markup = render({ accountName: 'werhua520', accountNote: '周三晚才能上' });

    expect(markup).toContain('备注');
    expect(markup).toContain('周三晚才能上');
  });

  it('没有备注时不渲染备注行', () => {
    const markup = render({ accountName: 'werhua520' });

    expect(markup).not.toContain('备注');
  });
});
