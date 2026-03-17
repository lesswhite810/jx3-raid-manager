export const getBaseServerName = (fullServer: string): string => {
  if (!fullServer) return '未知服务器';
  const parts = fullServer.trim().split(/\s+/);
  return parts[parts.length - 1] || fullServer;
};
