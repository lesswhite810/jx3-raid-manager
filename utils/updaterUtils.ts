export interface UpdateChannelInfo {
  isPortable: boolean;
  channel: 'installer' | 'portable';
  willInstallInPlace: boolean;
}

export interface GitHubReleaseAsset {
  name: string;
  browser_download_url: string;
}

export const normalizeReleaseNotes = (body?: string | null): string => {
  const normalized = body?.trim();
  return normalized ? normalized : '本次版本未提供更新说明。';
};

export const detectUpdateChannel = (executablePath: string, hasUninstallExecutable: boolean): UpdateChannelInfo => {
  const normalizedPath = executablePath.trim().toLowerCase();
  const looksLikePortableExe = normalizedPath.endsWith('.exe') && normalizedPath.includes('_v');
  const isPortable = looksLikePortableExe || !hasUninstallExecutable;

  return {
    isPortable,
    channel: isPortable ? 'portable' : 'installer',
    willInstallInPlace: !isPortable
  };
};

export const pickInstallerAsset = (assets: GitHubReleaseAsset[]): GitHubReleaseAsset | null => {
  return (
    assets.find(asset => /_x64-setup\.exe$/i.test(asset.name)) ??
    assets.find(asset => /setup\.exe$/i.test(asset.name)) ??
    null
  );
};

export const buildGitHubReleaseUrl = (repo: string, tagName: string): string => {
  return `https://github.com/${repo}/releases/tag/${tagName}`;
};
