import { readDir } from '@tauri-apps/api/fs';
import { GkpFileInfo, parseGkpFileName } from './gkpDirectoryScanner';
import { ParsedAccount } from './directoryParser';

export interface GameDirectoryScanResult {
  success: boolean;
  accounts: ParsedAccount[];
  gkpFiles: GkpFileInfo[];
  error?: string;
}

export interface ScanProgress {
  current: number;
  total: number;
  message: string;
}

export interface ScanOptions {
  gameDirectory: string;
  onProgress?: (progress: ScanProgress) => void;
  activeRoles?: Array<{ name: string; server: string; region: string }>;
}

const GKP_BASE_PATH = '\\interface\\my#data';
const USERDATA_BASE_PATH = '\\userdata';

export async function scanGameDirectory(options: ScanOptions): Promise<GameDirectoryScanResult> {
  try {
    const { gameDirectory, onProgress, activeRoles } = options;
    
    const accounts: ParsedAccount[] = [];
    const gkpFiles: GkpFileInfo[] = [];
    const errors: string[] = [];

    if (onProgress) {
      onProgress({ current: 0, total: 2, message: '正在扫描账号信息...' });
    }

    const accountsResult = await scanUserdataDirectory(gameDirectory);
    if (accountsResult.success) {
      accounts.push(...accountsResult.accounts);
    } else {
      errors.push(accountsResult.error || '扫描账号信息失败');
    }

    if (onProgress) {
      onProgress({ current: 1, total: 2, message: '正在扫描GKP文件...' });
    }

    const gkpResult = await scanGkpFilesDirectory(gameDirectory, activeRoles);
    if (gkpResult.success) {
      gkpFiles.push(...gkpResult.files);
    } else {
      errors.push(gkpResult.error || '扫描GKP文件失败');
    }

    if (onProgress) {
      onProgress({ current: 2, total: 2, message: '扫描完成' });
    }

    if (errors.length > 0) {
      return {
        success: true,
        accounts,
        gkpFiles,
        error: errors.join('; ')
      };
    }

    return {
      success: true,
      accounts,
      gkpFiles
    };
  } catch (error) {
    return {
      success: false,
      accounts: [],
      gkpFiles: [],
      error: `扫描失败: ${error instanceof Error ? error.message : String(error)}`
    };
  }
}

async function scanUserdataDirectory(gameDirectory: string): Promise<{ success: boolean; accounts: ParsedAccount[]; error?: string }> {
  try {
    const accounts: ParsedAccount[] = [];
    const userdataPath = `${gameDirectory}${USERDATA_BASE_PATH}`;

    const entries = await readDir(userdataPath) as any[];

    for (const entry of entries) {
      if (entry.children && entry.name) {
        const accountPath = `${userdataPath}\\${entry.name}`;
        
        const accountContext = {
          accountName: entry.name,
          roles: []
        };

        await parseAccountDirectory(accountPath, accountContext);

        if (accountContext.roles.length > 0) {
          accounts.push(accountContext);
        }
      }
    }

    return {
      success: true,
      accounts
    };
  } catch (error) {
    return {
      success: false,
      accounts: [],
      error: `扫描账号目录失败: ${error instanceof Error ? error.message : String(error)}`
    };
  }
}

async function parseAccountDirectory(accountPath: string, context: any): Promise<void> {
  try {
    const entries = await readDir(accountPath) as any[];

    for (const entry of entries) {
      if (entry.children && entry.name) {
        const regionPath = `${accountPath}\\${entry.name}`;
        
        await parseRegionDirectory(regionPath, context);
      }
    }
  } catch (error) {
    console.warn(`解析账号目录失败: ${accountPath}`, error);
  }
}

async function parseRegionDirectory(regionPath: string, context: any): Promise<void> {
  try {
    const entries = await readDir(regionPath) as any[];

    for (const entry of entries) {
      if (entry.children && entry.name) {
        const serverPath = `${regionPath}\\${entry.name}`;
        
        await parseServerDirectory(serverPath, context, entry.name);
      }
    }
  } catch (error) {
    console.warn(`解析大区目录失败: ${regionPath}`, error);
  }
}

async function parseServerDirectory(serverPath: string, context: any, regionName: string): Promise<void> {
  try {
    const entries = await readDir(serverPath) as any[];

    for (const entry of entries) {
      if (entry.children && entry.name) {
        context.roles.push({
          name: entry.name,
          region: regionName,
          server: entry.name
        });
      }
    }
  } catch (error) {
    console.warn(`解析服务器目录失败: ${serverPath}`, error);
  }
}

async function scanGkpFilesDirectory(gameDirectory: string, activeRoles?: Array<{ name: string; server: string; region: string }>): Promise<{ success: boolean; files: GkpFileInfo[]; error?: string }> {
  try {
    const files: GkpFileInfo[] = [];
    const gkpBasePath = `${gameDirectory}${GKP_BASE_PATH}`;
    const myDataEntries = await readDir(gkpBasePath);
    
    for (const entry of myDataEntries) {
      if (entry.name?.endsWith('@zhcn_hd')) {
        const userDataPath = `${gkpBasePath}\\${entry.name}\\userdata\\gkp`;
        
        try {
          const gkpEntries = await readDir(userDataPath);
          
          for (const gkpEntry of gkpEntries) {
            if (gkpEntry.name?.endsWith('.gkp.jx3dat')) {
              const fileInfo = parseGkpFileName(gkpEntry.name);
              
              if (fileInfo) {
                const fullPath = `${userDataPath}\\${gkpEntry.name}`;
                
                // 如果提供了角色列表，尝试匹配角色名
                let matchedRoleName: string | undefined;
                if (activeRoles && activeRoles.length > 0) {
                  // 从目录名中提取角色名（例如：342277519517081876@zhcn_hd -> 342277519517081876）
                  
                  // 遍历用户目录下的子目录，查找匹配的角色
                  try {
                    const userDirPath = `${gkpBasePath}\\${entry.name}`;
                    const subEntries = await readDir(userDirPath);
                    
                    for (const subEntry of subEntries) {
                      if (subEntry.name && subEntry.children) {
                        const matchedRole = activeRoles.find(role => role.name === subEntry.name);
                        if (matchedRole) {
                          matchedRoleName = matchedRole.name;
                          break;
                        }
                      }
                    }
                  } catch (error) {
                    // 忽略子目录读取错误，继续处理GKP文件
                  }
                }
                
                const gkpFile: GkpFileInfo = {
                  filePath: fullPath,
                  fileName: gkpEntry.name,
                  timestamp: fileInfo.timestamp!,
                  playerCount: fileInfo.playerCount!,
                  mapName: fileInfo.mapName!,
                  roleName: matchedRoleName
                };

                files.push(gkpFile);
              }
            }
          }
        } catch (error) {
          console.warn(`无法读取GKP目录 ${entry.name}:`, error);
        }
      }
    }
    
    return {
      success: true,
      files
    };
  } catch (error) {
    return {
      success: false,
      files: [],
      error: `扫描GKP文件失败: ${error instanceof Error ? error.message : String(error)}`
    };
  }
}

export async function validateGameDirectory(gameDirectory: string): Promise<boolean> {
  if (!gameDirectory || typeof gameDirectory !== 'string') {
    return false;
  }

  const windowsPathRegex = /^[a-zA-Z]:\\|^\\\\|^\//;
  if (!windowsPathRegex.test(gameDirectory)) {
    return false;
  }

  try {
    const entries = await readDir(gameDirectory) as any[];
    
    const hasValidStructure = entries.some((entry: any) => {
      const name = entry.name?.toLowerCase();
      return name === 'userdata' || name === 'interface';
    });

    return hasValidStructure;
  } catch (error) {
    console.warn('验证游戏目录失败:', error);
    return false;
  }
}
