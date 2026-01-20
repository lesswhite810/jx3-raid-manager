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
        return 0;
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
const sortRoles = (roles: Role[]): Role[] => {
    return [...roles].sort((a, b) => {
        const aDisabled = !!a.disabled;
        const bDisabled = !!b.disabled;

        if (aDisabled !== bDisabled) {
            return (aDisabled ? 1 : 0) - (bDisabled ? 1 : 0);
        }
        return 0;
    });
};
