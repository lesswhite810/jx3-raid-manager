import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { CloseConfirmDialog } from './CloseConfirmDialog';

const renderDialog = (isOpen = true): string =>
    renderToStaticMarkup(
        <CloseConfirmDialog isOpen={isOpen} onConfirm={vi.fn()} onCancel={vi.fn()} />
    );

describe('CloseConfirmDialog', () => {
    it('关闭状态下不渲染任何内容', () => {
        expect(renderDialog(false)).toBe('');
    });

    it('同时提供最小化到托盘与直接退出两个选项', () => {
        const markup = renderDialog();
        expect(markup).toContain('最小化到托盘');
        expect(markup).toContain('直接退出');
        expect(markup).toContain('要如何关闭剑网三副本管家？');
    });

    it('默认勾选记住选择，并说明可以在设置中改回每次询问', () => {
        const markup = renderDialog();
        expect(markup).toContain('type="checkbox"');
        expect(markup).toContain('checked=""');
        expect(markup).toContain('下次不再询问');
        expect(markup).toContain('改回每次询问');
    });

    it('提供取消入口，避免误触后无法返回主界面', () => {
        const markup = renderDialog();
        expect(markup).toContain('取消');
    });

    it('提交中禁用两个选项与取消按钮，避免重复点击', () => {
        const markup = renderToStaticMarkup(
            <CloseConfirmDialog isOpen onConfirm={vi.fn()} onCancel={vi.fn()} submitting />
        );
        const disabledCount = [...markup.matchAll(/disabled=""/g)].length;
        expect(disabledCount).toBeGreaterThanOrEqual(4);
    });
});
