import { Account, Role } from '../types';

interface ClosestCapableTarget {
    closest?: (selector: string) => unknown;
}

const ACCOUNT_REORDER_MIN_DURATION = 170;
const ACCOUNT_REORDER_MAX_DURATION = 280;
const ACCOUNT_REORDER_DURATION_PER_PIXEL = 0.2;

const getAccountSortOrder = (account: Account, fallbackIndex: number): number => {
    return typeof account.sortOrder === 'number' ? account.sortOrder : fallbackIndex;
};

export const canStartAccountDrag = (target: EventTarget | ClosestCapableTarget | null): boolean => {
    if (!target || typeof (target as ClosestCapableTarget).closest !== 'function') {
        return true;
    }

    return !(target as ClosestCapableTarget).closest?.('[data-no-account-drag="true"]');
};

export const normalizeAccountSortOrder = (accounts: Account[]): Account[] => {
    return accounts.map((account, index) => ({
        ...account,
        sortOrder: index,
    }));
};

export const getAccountReorderAnimationDuration = (distance: number): number => {
    const normalizedDistance = Math.abs(distance);
    const duration = ACCOUNT_REORDER_MIN_DURATION + normalizedDistance * ACCOUNT_REORDER_DURATION_PER_PIXEL;

    return Math.max(
        ACCOUNT_REORDER_MIN_DURATION,
        Math.min(ACCOUNT_REORDER_MAX_DURATION, Math.round(duration)),
    );
};

export const reorderAccounts = (
    accounts: Account[],
    draggedAccountId: string,
    targetAccountId: string,
): Account[] => {
    if (draggedAccountId === targetAccountId) {
        return normalizeAccountSortOrder(accounts);
    }

    const draggedIndex = accounts.findIndex(account => account.id === draggedAccountId);
    const targetIndex = accounts.findIndex(account => account.id === targetAccountId);

    if (draggedIndex === -1 || targetIndex === -1) {
        return normalizeAccountSortOrder(accounts);
    }

    const reorderedAccounts = [...accounts];
    const [draggedAccount] = reorderedAccounts.splice(draggedIndex, 1);
    reorderedAccounts.splice(targetIndex, 0, draggedAccount);

    return normalizeAccountSortOrder(reorderedAccounts);
};

/**
 * Sorts accounts by persisted manual order first.
 * Disabled state only acts as a tie-breaker when sortOrder is identical.
 */
export const sortAccounts = (accounts: Account[]): Account[] => {
    const originalOrderMap = new Map(accounts.map((account, index) => [account.id, index]));

    const sortedAccounts = [...accounts].sort((a, b) => {
        const aSortOrder = getAccountSortOrder(a, originalOrderMap.get(a.id) ?? 0);
        const bSortOrder = getAccountSortOrder(b, originalOrderMap.get(b.id) ?? 0);

        if (aSortOrder !== bSortOrder) {
            return aSortOrder - bSortOrder;
        }

        const aDisabled = !!a.disabled;
        const bDisabled = !!b.disabled;
        if (aDisabled !== bDisabled) {
            return (aDisabled ? 1 : 0) - (bDisabled ? 1 : 0);
        }

        // Secondary sort by accountName alphabetically to ensure stable ordering
        return a.accountName.localeCompare(b.accountName, 'zh-CN');
    });

    // Sort roles within each account
    return normalizeAccountSortOrder(sortedAccounts.map(account => ({
        ...account,
        roles: sortRoles(account.roles || [])
    })));
};

/**
 * Sorts roles based on disabled status.
 * Enabled roles first, disabled roles last.
 */
/**
 * Sorts roles based on:
 * 1. Disabled status (Enabled first)
 * 2. Configuration status (Both Sect & Score > Either > None)
 * 3. Equipment Score (Descending) when both configured
 */
export const sortRoles = (roles: Role[]): Role[] => {
    return [...roles].sort((a, b) => {
        // 1. Sort by Disabled status
        const aDisabled = !!a.disabled;
        const bDisabled = !!b.disabled;
        if (aDisabled !== bDisabled) {
            return (aDisabled ? 1 : 0) - (bDisabled ? 1 : 0);
        }

        // 2. Sort by Configuration status
        // Definition of "Configured": Has sect AND/OR equipmentScore
        // Priority:
        // 2: Has BOTH Sect and Score
        // 1: Has Either
        // 0: Has Neither
        const getConfigurationScore = (r: Role) => {
            let score = 0;
            if (r.sect) score++;
            if (r.equipmentScore !== undefined && r.equipmentScore !== null) score++;
            return score;
        };

        const aConfigScore = getConfigurationScore(a);
        const bConfigScore = getConfigurationScore(b);

        if (aConfigScore !== bConfigScore) {
            return bConfigScore - aConfigScore; // Higher score first
        }

        // 3. Tie-breaker: Equipment Score (High to Low) if available
        if (a.equipmentScore !== undefined && b.equipmentScore !== undefined && a.equipmentScore !== b.equipmentScore) {
            return b.equipmentScore - a.equipmentScore;
        }

        // 4. Final tie-breaker: Role name alphabetically
        return a.name.localeCompare(b.name, 'zh-CN');
    });
};
