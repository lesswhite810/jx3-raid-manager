import { Account, Role } from '../types';

/**
 * Sorts accounts and their roles based on disabled status.
 * Enabled items appear first, disabled items appear last.
 * @param accounts The list of accounts to sort
 * @returns A new sorted list of accounts
 */
export const sortAccounts = (accounts: Account[]): Account[] => {
    // Sort accounts: Enabled first, Disabled last
    const sortedAccounts = [...accounts].sort((a, b) => {
        // Treat undefined disabled as false (enabled)
        const aDisabled = !!a.disabled;
        const bDisabled = !!b.disabled;

        if (aDisabled !== bDisabled) {
            return (aDisabled ? 1 : 0) - (bDisabled ? 1 : 0);
        }

        // Secondary sort by accountName alphabetically to ensure stable ordering
        return a.accountName.localeCompare(b.accountName, 'zh-CN');
    });

    // Sort roles within each account
    return sortedAccounts.map(account => ({
        ...account,
        roles: sortRoles(account.roles || [])
    }));
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
