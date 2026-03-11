import { Role } from '../types';
import { db } from './db';

export interface DeleteDirectoryResult {
  deleted: boolean;
  path: string;
}

const USERDATA_DIRECTORY = 'userdata';
const PATH_SEPARATOR = '\\';

type RoleDirectoryTarget = Pick<Role, 'name' | 'region' | 'server'>;

const normalizePathSegment = (segment: string): string => segment.replace(/^[\\/]+|[\\/]+$/gu, '');

const trimTrailingSeparators = (targetPath: string): string => targetPath.replace(/[\\/]+$/gu, '');

const joinWindowsPath = (...segments: string[]): string => {
  return segments
    .map(normalizePathSegment)
    .filter(Boolean)
    .join(PATH_SEPARATOR);
};

const deleteLoggedDirectory = (path: string, targetType: string): Promise<DeleteDirectoryResult> => {
  return db.deleteDirectory(trimTrailingSeparators(path), targetType);
};

export const buildAccountDirectoryPath = (gameDirectory: string, accountName: string): string => {
  return joinWindowsPath(trimTrailingSeparators(gameDirectory), USERDATA_DIRECTORY, accountName);
};

export const buildRoleDirectoryPath = (
  gameDirectory: string,
  accountName: string,
  role: RoleDirectoryTarget
): string => {
  return joinWindowsPath(
    trimTrailingSeparators(gameDirectory),
    USERDATA_DIRECTORY,
    accountName,
    role.region,
    role.server,
    role.name
  );
};

export async function deleteDirectoryIfExists(
  path: string,
  targetType: string
): Promise<DeleteDirectoryResult> {
  return deleteLoggedDirectory(path, targetType);
}

export const deleteAccountDirectory = (
  gameDirectory: string,
  accountName: string
): Promise<DeleteDirectoryResult> => {
  return deleteDirectoryIfExists(buildAccountDirectoryPath(gameDirectory, accountName), '\u8d26\u53f7');
};

export const deleteRoleDirectory = (
  gameDirectory: string,
  accountName: string,
  role: RoleDirectoryTarget
): Promise<DeleteDirectoryResult> => {
  return deleteDirectoryIfExists(buildRoleDirectoryPath(gameDirectory, accountName, role), '\u89d2\u8272');
};
