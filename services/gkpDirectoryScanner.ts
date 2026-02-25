import { readDir } from '@tauri-apps/api/fs';

export interface GkpFileInfo {
  filePath: string;
  fileName: string;
  timestamp: number;
  playerCount: number;
  mapName: string;
  roleName?: string;
}

export interface ScanResult {
  success: boolean;
  files: GkpFileInfo[];
  error?: string;
  logs?: string[];
}

export interface ScanOptions {
  gameDirectory: string;
  activeRoles?: Array<{ name: string; server: string; region: string }>;
}

const GKP_FILE_PATTERN = /^(\d{4}-\d{2}-\d{2}-\d{2}-\d{2}-\d{2})_(\d+)人(.+)\.gkp\.jx3dat$/;

export function parseGkpFileName(fileName: string): Partial<GkpFileInfo> | null {
  const match = fileName.match(GKP_FILE_PATTERN);
  if (!match) {
    return null;
  }

  const [, timestampStr, playerCountStr, mapName] = match;
  const [year, month, day, hour, minute, second] = timestampStr.split('-').map(Number);
  const timestamp = new Date(year, month - 1, day, hour, minute, second).getTime();

  return {
    fileName,
    timestamp,
    playerCount: parseInt(playerCountStr, 10),
    mapName: mapName.trim()
  };
}

export async function scanGkpDirectory(options: ScanOptions): Promise<ScanResult> {
  try {
    const { gameDirectory, activeRoles = [] } = options;
    
    const logs: string[] = [];
    
    const addLog = (level: 'DEBUG' | 'INFO' | 'WARN' | 'ERROR' | 'FATAL', message: string) => {
      const timestamp = new Date().toLocaleTimeString('zh-CN', { hour12: false });
      const logEntry = `[${timestamp}] [${level}] ${message}`;
      logs.push(logEntry);
      
      // 根据日志级别输出到不同的控制台方法
      switch (level) {
        case 'DEBUG':
          console.log(logEntry);
          break;
        case 'INFO':
          console.info(logEntry);
          break;
        case 'WARN':
          console.warn(logEntry);
          break;
        case 'ERROR':
          console.error(logEntry);
          break;
        case 'FATAL':
          console.error(logEntry);
          break;
      }
    };
    
    addLog('INFO', '=== GKP扫描开始 ===');
    addLog('DEBUG', `游戏目录: ${gameDirectory}`);
    addLog('DEBUG', `角色列表 (activeRoles): ${JSON.stringify(activeRoles)}`);
    addLog('INFO', `角色数量: ${activeRoles.length}`);
    
    const files: GkpFileInfo[] = [];
    const errors: string[] = [];

    const interfacePath = `${gameDirectory}\\interface\\my#data`;
    addLog('DEBUG', `扫描路径: ${interfacePath}`);
    
    try {
      const myDataEntries = await readDir(interfacePath);
      addLog('DEBUG', `my#data目录条目数: ${myDataEntries.length}`);
      addLog('DEBUG', `myDataEntries: ${JSON.stringify(myDataEntries.map(e => ({ name: e.name, children: e.children })))}`);
      
      // 如果没有提供角色列表，则扫描所有目录（保持向后兼容）
      if (activeRoles.length === 0) {
        addLog('WARN', '未提供角色列表，扫描所有目录');
        for (const entry of myDataEntries) {
          if (entry.name?.endsWith('@zhcn_hd')) {
            const userDataPath = `${interfacePath}\\${entry.name}\\userdata\\gkp`;
            
            try {
              const gkpEntries = await readDir(userDataPath);
              
              for (const gkpEntry of gkpEntries) {
                if (gkpEntry.name?.endsWith('.gkp.jx3dat')) {
                  const fileInfo = parseGkpFileName(gkpEntry.name);
                  
                  if (fileInfo) {
                    const fullPath = `${userDataPath}\\${gkpEntry.name}`;
                    
                    const gkpFile: GkpFileInfo = {
                      filePath: fullPath,
                      fileName: gkpEntry.name,
                      timestamp: fileInfo.timestamp!,
                      playerCount: fileInfo.playerCount!,
                      mapName: fileInfo.mapName!
                    };

                    files.push(gkpFile);
                  }
                }
              }
            } catch (error) {
              addLog('ERROR', `无法读取目录 ${entry.name}: ${error instanceof Error ? error.message : String(error)}`);
              errors.push(`无法读取目录 ${entry.name}: ${error instanceof Error ? error.message : String(error)}`);
            }
          }
        }
      } else {
        addLog('INFO', '使用角色列表进行匹配');
        // 基于角色列表进行目录结构匹配
        for (const entry of myDataEntries) {
          addLog('DEBUG', `\n处理用户目录: ${entry.name}`);
          
          if (!entry.name?.endsWith('@zhcn_hd')) {
            addLog('WARN', `  跳过: 不是@zhcn_hd目录`);
            continue;
          }
          
          const userDirectoryPath = `${interfacePath}\\${entry.name}`;
          addLog('DEBUG', `  用户目录路径: ${userDirectoryPath}`);
          
          try {
            // 遍历用户目录下的子目录（角色目录）
            const subEntries = await readDir(userDirectoryPath);
            addLog('DEBUG', `  子目录数量: ${subEntries.length}`);
            addLog('DEBUG', `  子目录列表: ${JSON.stringify(subEntries.map(e => ({ name: e.name, children: e.children })))}`);
            
            // 检查是否存在匹配的角色目录
            let matchedRoleName: string | null = null;
            
            for (const subEntry of subEntries) {
              addLog('DEBUG', `    检查子目录: ${subEntry.name}, children: ${subEntry.children}`);
              
              if (subEntry.name && subEntry.children) {
                // 检查子目录名称是否在角色列表中
                const matchedRole = activeRoles.find(role => role.name === subEntry.name);
                
                addLog('DEBUG', `      匹配检查: "${subEntry.name}" vs 角色列表`);
                addLog('DEBUG', `      匹配结果: ${matchedRole ? `✓ 找到匹配角色: ${matchedRole.name}` : '✗ 未匹配'}`);
                
                if (matchedRole) {
                  matchedRoleName = matchedRole.name;
                  addLog('DEBUG', `      设置matchedRoleName: ${matchedRoleName}`);
                  break; // 找到匹配角色后立即跳出循环
                }
              }
            }
            
            addLog('INFO', `  最终matchedRoleName: ${matchedRoleName}`);
            
            // 如果找到匹配的角色，则解析该用户目录下的GKP文件
            if (matchedRoleName) {
              addLog('INFO', `  ✓ 找到匹配角色，开始解析GKP文件`);
              const userDataPath = `${userDirectoryPath}\\userdata\\gkp`;
              addLog('DEBUG', `  GKP路径: ${userDataPath}`);
              
              try {
                const gkpEntries = await readDir(userDataPath);
                addLog('DEBUG', `  GKP文件数量: ${gkpEntries.length}`);
                
                for (const gkpEntry of gkpEntries) {
                  addLog('DEBUG', `    处理GKP文件: ${gkpEntry.name}`);
                  
                  if (gkpEntry.name?.endsWith('.gkp.jx3dat')) {
                    const fileInfo = parseGkpFileName(gkpEntry.name);
                    
                    if (fileInfo) {
                      const fullPath = `${userDataPath}\\${gkpEntry.name}`;
                      addLog('INFO', `      ✓ 解析成功: ${gkpEntry.name}`);
                      
                      const gkpFile: GkpFileInfo = {
                        filePath: fullPath,
                        fileName: gkpEntry.name,
                        timestamp: fileInfo.timestamp!,
                        playerCount: fileInfo.playerCount!,
                        mapName: fileInfo.mapName!,
                        roleName: matchedRoleName
                      };

                      files.push(gkpFile);
                      addLog('DEBUG', `      添加到结果列表，当前总数: ${files.length}`);
                    } else {
                      addLog('WARN', `      ✗ 文件名解析失败: ${gkpEntry.name}`);
                    }
                  }
                }
              } catch (error) {
                addLog('ERROR', `  ✗ 无法读取GKP目录: ${error instanceof Error ? error.message : String(error)}`);
                errors.push(`无法读取GKP目录 ${entry.name}: ${error instanceof Error ? error.message : String(error)}`);
              }
            } else {
              addLog('WARN', `  ✗ 未找到匹配角色，跳过此用户目录`);
            }
          } catch (error) {
            addLog('ERROR', `  ✗ 无法读取用户目录 ${entry.name}: ${error instanceof Error ? error.message : String(error)}`);
            errors.push(`无法读取用户目录 ${entry.name}: ${error instanceof Error ? error.message : String(error)}`);
          }
        }
      }
    } catch (error) {
      addLog('FATAL', `✗ 无法读取my#data目录: ${error instanceof Error ? error.message : String(error)}`);
      errors.push(`无法读取my#data目录: ${error instanceof Error ? error.message : String(error)}`);
    }

    addLog('INFO', `\n=== 扫描结果汇总 ===`);
    addLog('INFO', `找到的GKP文件总数: ${files.length}`);
    addLog('INFO', `匹配角色的文件数: ${files.filter(f => f.roleName).length}`);
    addLog('INFO', `未匹配角色的文件数: ${files.filter(f => !f.roleName).length}`);
    addLog('INFO', `错误数量: ${errors.length}`);
    if (errors.length > 0) {
      addLog('ERROR', `错误列表: ${JSON.stringify(errors)}`);
    }
    addLog('INFO', '=== GKP扫描结束 ===\n');

    return {
      success: true,
      files,
      logs
    };
  } catch (error) {
    return {
      success: false,
      files: [],
      logs: [],
      error: `扫描失败: ${error instanceof Error ? error.message : String(error)}`
    };
  }
}

export function isGkpFile(fileName: string): boolean {
  return GKP_FILE_PATTERN.test(fileName);
}

export function formatGkpFileInfo(file: GkpFileInfo): string {
  const date = new Date(file.timestamp);
  const formattedDate = date.toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit'
  });

  return `${formattedDate} - ${file.playerCount}人${file.mapName}`;
}
