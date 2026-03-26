import { RaidRecord } from '../types';

export const SPECIAL_DROP_DEFINITIONS = [
  { field: 'hasXuanjing', label: '玄晶' },
  { field: 'hasMaJu', label: '马具' },
  { field: 'hasPet', label: '宠物' },
  { field: 'hasPendant', label: '挂件' },
  { field: 'hasMount', label: '坐骑' },
  { field: 'hasAppearance', label: '外观' },
  { field: 'hasTitle', label: '称号' },
  { field: 'hasSecretBook', label: '秘籍' },
] as const;

export type SpecialDropField = (typeof SPECIAL_DROP_DEFINITIONS)[number]['field'];
export type SpecialDropType = (typeof SPECIAL_DROP_DEFINITIONS)[number]['label'];

export interface SpecialDropRecord {
  id: string;
  recordId: string;
  date: string | number;
  raidName: string;
  notes?: string;
  type: SpecialDropType;
  roleId: string;
  accountId: string;
  roleName?: string;
  server?: string;
}

const getRecordTime = (date: string | number): number => {
  return typeof date === 'number' ? date : new Date(date).getTime();
};

export const buildSpecialDropRecords = (
  records: RaidRecord[],
  periodStartTime: number | null,
): SpecialDropRecord[] => {
  const drops: SpecialDropRecord[] = [];

  records.forEach(record => {
    if (periodStartTime !== null && getRecordTime(record.date) < periodStartTime) {
      return;
    }

    SPECIAL_DROP_DEFINITIONS.forEach(definition => {
      if (record[definition.field] !== true) {
        return;
      }

      drops.push({
        id: `${record.id}-${definition.field}`,
        recordId: record.id,
        date: record.date,
        raidName: record.raidName,
        notes: record.notes,
        type: definition.label,
        roleId: record.roleId || record.accountId,
        accountId: record.accountId,
        roleName: record.roleName,
        server: record.server,
      });
    });
  });

  return drops;
};
