import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { ConfigSupportPanel } from './ConfigSupportPanel';

const renderPanel = (): string => renderToStaticMarkup(<ConfigSupportPanel />);

describe('ConfigSupportPanel', () => {
    it('只展示一张本地打包的微信赞赏码，不加载远端资源', () => {
        const markup = renderPanel();
        const imageSources = [...markup.matchAll(/src="([^"]+)"/g)].map(match => match[1]);
        expect(imageSources).toEqual(['/wechat-reward-qr.png']);
        expect(markup).toContain('alt="微信赞赏码"');
    });

    it('不出现金额档位、支付宝或付费权益话术', () => {
        const markup = renderPanel();
        expect(markup).not.toContain('支付宝');
        expect(markup).not.toContain('¥');
        expect(markup).not.toContain('支付');
        expect(markup).not.toContain('会员');
        expect(markup).not.toContain('订阅');
        expect(markup).not.toContain('解锁额外');
    });

    it('明确声明完全免费且纯自愿', () => {
        const markup = renderPanel();
        expect(markup).toContain('完全免费');
        expect(markup).toContain('自愿');
        expect(markup).toContain('扫码不解锁任何功能');
    });

    it('默认不渲染放大层，仅提供放大入口', () => {
        const markup = renderPanel();
        // 放大层走 createPortal，默认关闭时不应出现在静态输出里
        expect(markup).not.toContain('role="dialog"');
        expect(markup).toContain('aria-haspopup="dialog"');
        expect(markup).toContain('放大查看微信赞赏码');
    });
});
