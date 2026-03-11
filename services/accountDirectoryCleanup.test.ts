import { beforeEach, describe, expect, it, vi } from 'vitest';
import { Role } from '../types';

const deleteDirectoryMock = vi.fn();

vi.mock('./db', () => ({
  db: {
    deleteDirectory: deleteDirectoryMock
  }
}));

import {
  buildAccountDirectoryPath,
  buildRoleDirectoryPath,
  deleteAccountDirectory,
  deleteDirectoryIfExists,
  deleteRoleDirectory
} from './accountDirectoryCleanup';

const sampleRole: Role = {
  id: 'role-1',
  name: '\u89d2\u8272A',
  region: '\u7535\u4e00',
  server: '\u957f\u5b89',
  sect: '\u7eaf\u9633'
};

const gameDirectory = 'D:\\JX3\\bin\\zhcn_hd';
const accountDirectory = 'D:\\JX3\\bin\\zhcn_hd\\userdata\\test-account';
const roleDirectory = 'D:\\JX3\\bin\\zhcn_hd\\userdata\\test-account\\\u7535\u4e00\\\u957f\u5b89\\\u89d2\u8272A';

describe('accountDirectoryCleanup', () => {
  beforeEach(() => {
    deleteDirectoryMock.mockReset();
  });

  it('buildAccountDirectoryPath appends userdata and account name', () => {
    expect(buildAccountDirectoryPath(`${gameDirectory}\\`, 'test-account')).toBe(accountDirectory);
  });

  it('buildRoleDirectoryPath appends region, server, and role name', () => {
    expect(buildRoleDirectoryPath(gameDirectory, 'test-account', sampleRole)).toBe(roleDirectory);
  });

  it('deleteDirectoryIfExists forwards full path to Tauri', async () => {
    deleteDirectoryMock.mockResolvedValue({
      deleted: false,
      path: accountDirectory
    });

    await expect(
      deleteDirectoryIfExists(accountDirectory, '\u8d26\u53f7')
    ).resolves.toEqual({
      deleted: false,
      path: accountDirectory
    });

    expect(deleteDirectoryMock).toHaveBeenCalledWith(accountDirectory, '\u8d26\u53f7');
  });

  it('deleteAccountDirectory deletes the account directory', async () => {
    deleteDirectoryMock.mockResolvedValue({
      deleted: true,
      path: accountDirectory
    });

    await deleteAccountDirectory(gameDirectory, 'test-account');

    expect(deleteDirectoryMock).toHaveBeenCalledWith(accountDirectory, '\u8d26\u53f7');
  });

  it('deleteRoleDirectory deletes the role directory', async () => {
    deleteDirectoryMock.mockResolvedValue({
      deleted: true,
      path: roleDirectory
    });

    await deleteRoleDirectory(gameDirectory, 'test-account', sampleRole);

    expect(deleteDirectoryMock).toHaveBeenCalledWith(roleDirectory, '\u89d2\u8272');
  });

  it('deleteDirectoryIfExists returns the Tauri result', async () => {
    deleteDirectoryMock.mockResolvedValue({
      deleted: true,
      path: accountDirectory
    });

    await expect(
      deleteDirectoryIfExists(accountDirectory, '\u8d26\u53f7')
    ).resolves.toEqual({
      deleted: true,
      path: accountDirectory
    });
  });

  it('deleteDirectoryIfExists propagates Tauri failures', async () => {
    deleteDirectoryMock.mockRejectedValue(new Error('\u5220\u9664\u5931\u8d25'));

    await expect(
      deleteDirectoryIfExists(accountDirectory, '\u8d26\u53f7')
    ).rejects.toThrow('\u5220\u9664\u5931\u8d25');
  });
});
