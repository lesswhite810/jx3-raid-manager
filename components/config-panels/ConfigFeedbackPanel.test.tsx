import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { ConfigFeedbackPanel } from './ConfigFeedbackPanel';

const renderPanel = (): string => renderToStaticMarkup(<ConfigFeedbackPanel />);

describe('ConfigFeedbackPanel', () => {
    it('只展示一张本地打包的 QQ 群二维码，不加载远端资源', () => {
        const markup = renderPanel();
        const imageSources = [...markup.matchAll(/src="([^"]+)"/g)].map(match => match[1]);
        expect(imageSources).toEqual(['/qq-group-qr.png']);
        expect(markup).toContain('alt="QQ群二维码"');
    });

    it('同时给出群号，方便直接搜索加入', () => {
        const markup = renderPanel();
        expect(markup).toContain('1085903108');
        expect(markup).toContain('群号');
    });

    it('默认不渲染放大层，仅提供放大入口', () => {
        const markup = renderPanel();
        // 放大层走 createPortal，默认关闭时不应出现在静态输出里
        expect(markup).not.toContain('role="dialog"');
        expect(markup).toContain('aria-haspopup="dialog"');
        expect(markup).toContain('放大查看 QQ 群二维码');
    });

    it('图片尺寸随弹窗高度收缩，避免默认尺寸出现滚动条', () => {
        const markup = renderPanel();
        expect(markup).toContain('h-[min(300px,calc(90vh_-_310px))]');
        expect(markup).toContain('w-[min(300px,calc(90vh_-_310px))]');
    });
});
