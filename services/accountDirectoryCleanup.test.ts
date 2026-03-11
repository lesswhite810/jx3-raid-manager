import { beforeEach, describe, expect, it, vi } from 'vitest';
import { Role } from '../types';

const { deleteDirectoryMock } = vi.hoisted(() => ({
  deleteDirectoryMock: vi.fn()
}));

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
  name: '角色A',
  region: '电一',
  server: '长安',
  sect: '纯阳'
};

const gameDirectory = 'D:\\JX3\\bin\\zhcn_hd';
const accountDirectory = 'D:\\JX3\\bin\\zhcn_hd\\userdata\\test-account';
const roleDirectory = 'D:\\JX3\\bin\\zhcn_hd\\userdata\\test-account\\电一\\长安\\角色A';

describe('accountDirectoryCleanup', () => {
  beforeEach(() => {
    deleteDirectoryMock.mockReset();
  });

  it('buildAccountDirectoryPath appends userdata and account name', () => {
    expect(buildAccountDirectoryPath('D:\\JX3\\bin\\zhcn_hd\\', 'test-account')).toBe(accountDirectory);
  });

  it('buildAccountDirectoryPath accepts SeasunGame install root', () => {
    expect(buildAccountDirectoryPath('D:\\SeasunGame', 'test-account')).toBe(
      'D:\\SeasunGame\\Game\\JX3\\bin\\zhcn_hd\\userdata\\test-account'
    );
  });

  it('buildRoleDirectoryPath appends region, server, and role name', () => {
    expect(buildRoleDirectoryPath(gameDirectory, 'test-account', sampleRole)).toBe(roleDirectory);
  });

  it('deleteDirectoryIfExists forwards full path to Tauri', async () => {
    deleteDirectoryMock.mockResolvedValue({
      deleted: false,
      path: accountDirectory
    });

    await expect(deleteDirectoryIfExists(accountDirectory, '账号')).resolves.toEqual({
      deleted: false,
      path: accountDirectory
    });

    expect(deleteDirectoryMock).toHaveBeenCalledWith(accountDirectory, '账号');
  });

  it('deleteAccountDirectory deletes the account directory', async () => {
    deleteDirectoryMock.mockResolvedValue({
      deleted: true,
      path: accountDirectory
    });

    await deleteAccountDirectory(gameDirectory, 'test-account');

    expect(deleteDirectoryMock).toHaveBeenCalledWith(accountDirectory, '账号');
  });

  it('deleteRoleDirectory deletes the role directory', async () => {
    deleteDirectoryMock.mockResolvedValue({
      deleted: true,
      path: roleDirectory
    });

    await deleteRoleDirectory(gameDirectory, 'test-account', sampleRole);

    expect(deleteDirectoryMock).toHaveBeenCalledWith(roleDirectory, '角色');
  });

  it('deleteDirectoryIfExists returns the Tauri result', async () => {
    deleteDirectoryMock.mockResolvedValue({
      deleted: true,
      path: accountDirectory
    });

    await expect(deleteDirectoryIfExists(accountDirectory, '账号')).resolves.toEqual({
      deleted: true,
      path: accountDirectory
    });
  });

  it('deleteDirectoryIfExists propagates Tauri failures', async () => {
    deleteDirectoryMock.mockRejectedValue(new Error('删除失败'));

    await expect(deleteDirectoryIfExists(accountDirectory, '账号')).rejects.toThrow('删除失败');
  });
});
