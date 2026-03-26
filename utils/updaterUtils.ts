export interface UpdateChannelInfo {
  isPortable: boolean;
  channel: 'installer' | 'portable';
  willInstallInPlace: boolean;
}

export interface GitHubReleaseAsset {
  name: string;
  browser_download_url: string;
}

const LEGACY_OFFSET_DATE_TIME_PATTERN =
  /^(\d{4}-\d{2}-\d{2}) (\d{2}:\d{2}:\d{2}(?:\.\d+)?) ([+-]\d{2}:\d{2}):\d{2}$/;

export const normalizeReleaseNotes = (body?: string | null): string => {
  const normalized = body?.trim();
  return normalized ? normalized : '本次版本未提供更新说明。';
};

export const formatUpdatePubDate = (value?: string | null): string | null => {
  const normalized = value?.trim();
  if (!normalized) {
    return null;
  }

  const parsedDate = new Date(normalized);
  if (!Number.isNaN(parsedDate.getTime())) {
    return parsedDate.toLocaleString('zh-CN');
  }

  const legacyMatch = normalized.match(LEGACY_OFFSET_DATE_TIME_PATTERN);
  if (!legacyMatch) {
    return null;
  }

  const [, datePart, timePart, offsetPart] = legacyMatch;
  const legacyParsedDate = new Date(`${datePart}T${timePart}${offsetPart}`);

  return Number.isNaN(legacyParsedDate.getTime())
    ? null
    : legacyParsedDate.toLocaleString('zh-CN');
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
