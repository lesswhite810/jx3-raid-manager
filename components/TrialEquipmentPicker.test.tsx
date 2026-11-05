import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { TrialEquipmentCatalogState, TrialEquipmentPicker } from './TrialEquipmentPicker';

const renderPicker = (catalogState: TrialEquipmentCatalogState, selectedEquipmentId = '51443', isSaving = false): string => (
    renderToStaticMarkup(
        <TrialEquipmentPicker
            equipments={[]}
            catalogState={catalogState}
            selectedEquipmentId={selectedEquipmentId}
            onSelect={vi.fn()}
            onClear={vi.fn()}
            isSaving={isSaving}
        />,
    )
);

describe('TrialEquipmentPicker unavailable catalog actions', () => {
    it.each<TrialEquipmentCatalogState>(['loading', 'error', 'empty'])('keeps the clear action when %s', catalogState => {
        const markup = renderPicker(catalogState);
        expect(markup).toContain('清空当前牌位装备');
        expect(markup).not.toContain('disabled=""');
    });

    it('disables the clear action only while saving', () => {
        expect(renderPicker('error', '51443', true)).toContain('disabled=""');
        expect(renderPicker('empty', '')).not.toContain('清空当前牌位装备');
    });
});
