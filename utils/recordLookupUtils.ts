import { Account, AccountType } from '../types';

export interface RoleInfo {
  roleName: string;
  server: string;
}

export interface EquipmentLike {
  ID?: string | number;
  [key: string]: unknown;
}

export interface VisibleRecordRange {
  startIndex: number;
  endIndex: number;
  topPadding: number;
  bottomPadding: number;
}

export interface VisibleRecordRangeInput {
  totalCount: number;
  scrollTop: number;
  viewportHeight: number;
  rowHeight: number;
  overscan: number;
}

export const getRoleInfoKey = (accountId: string, roleId: string): string => `${accountId}::${roleId}`;

export const buildRoleInfoLookup = (accounts: Account[]): Map<string, RoleInfo> => {
  const lookup = new Map<string, RoleInfo>();

  accounts.forEach(account => {
    const roles = Array.isArray(account.roles) ? account.roles : [];
    roles.forEach(role => {
      lookup.set(getRoleInfoKey(account.id, role.id), {
        roleName: role.name,
        server: `${role.region} ${role.server}`.trim()
      });
    });
  });

  return lookup;
};

export const buildClientAccountIdSet = (accounts: Account[]): Set<string> => {
  return new Set(
    accounts
      .filter(account => account.type === AccountType.CLIENT && !account.disabled)
      .map(account => account.id)
  );
};

const normalizeEquipmentId = (id: string | number | undefined): string => {
  if (id === undefined || id === null) {
    return '';
  }
  return String(id).trim();
};

export const buildEquipmentLookup = <T extends EquipmentLike>(equipments: T[]): Map<string, T> => {
  const lookup = new Map<string, T>();

  equipments.forEach(equipment => {
    const id = normalizeEquipmentId(equipment.ID);
    if (!id) {
      return;
    }

    lookup.set(id, equipment);

    if (id.includes('_')) {
      const numericPart = id.split('_').at(-1);
      if (numericPart) {
        lookup.set(numericPart, equipment);
      }
    }
  });

  return lookup;
};

export const getEquipmentById = <T extends EquipmentLike>(
  lookup: Map<string, T>,
  id: string | number | undefined
): T | null => {
  const normalizedId = normalizeEquipmentId(id);
  if (!normalizedId) {
    return null;
  }

  const directMatch = lookup.get(normalizedId);
  if (directMatch) {
    return directMatch;
  }

  if (!normalizedId.includes('_')) {
    return null;
  }

  const numericPart = normalizedId.split('_').at(-1);
  return numericPart ? lookup.get(numericPart) ?? null : null;
};

export const getVisibleRecordRange = ({
  totalCount,
  scrollTop,
  viewportHeight,
  rowHeight,
  overscan
}: VisibleRecordRangeInput): VisibleRecordRange => {
  if (totalCount <= 0 || rowHeight <= 0 || viewportHeight <= 0) {
    return { startIndex: 0, endIndex: 0, topPadding: 0, bottomPadding: 0 };
  }

  const safeScrollTop = Math.max(0, scrollTop);
  const safeOverscan = Math.max(0, overscan);
  const firstVisibleIndex = Math.floor(safeScrollTop / rowHeight);
  const visibleCount = Math.ceil(viewportHeight / rowHeight);
  const startIndex = Math.max(0, firstVisibleIndex - safeOverscan);
  const endIndex = Math.min(totalCount, firstVisibleIndex + visibleCount + safeOverscan);

  return {
    startIndex,
    endIndex,
    topPadding: startIndex * rowHeight,
    bottomPadding: Math.max(0, (totalCount - endIndex) * rowHeight)
  };
};
