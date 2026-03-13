import { AccountType } from '../types';

export interface SearchableRaidRole {
  id: string;
  name: string;
  server: string;
  region: string;
  sect: string;
  accountName: string;
  accountNote?: string;
}

export const getClientAccountNote = (
  accountType: AccountType | string | undefined,
  note?: string
): string | undefined => {
  if (accountType !== AccountType.CLIENT) {
    return undefined;
  }

  const trimmedNote = note?.trim();
  return trimmedNote ? trimmedNote : undefined;
};

export const filterRaidRoles = <T extends SearchableRaidRole>(roles: T[], searchTerm: string): T[] => {
  const normalizedSearch = searchTerm.trim().toLowerCase();
  if (!normalizedSearch) {
    return roles;
  }

  return roles.filter(role => {
    return [
      role.name,
      role.server,
      role.region,
      role.sect,
      role.accountName,
      role.accountNote
    ].some(field => field?.toLowerCase().includes(normalizedSearch));
  });
};
