import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import path from 'node:path';

const LOCAL_FAST_BUILD_DISABLE_ENV = 'JX3_TAURI_FULL_LOCAL_BUILD';
const DEFAULT_WINDOWS_TARGET = 'x86_64-pc-windows-msvc';

function isBuildCommand(args) {
  return args[0] === 'build';
}

function hasBundleFlag(args) {
  return args.includes('--bundles') || args.includes('--no-bundle');
}

function hasDebugFlag(args) {
  return args.includes('--debug') || args.includes('-d');
}

function shouldBundle(env) {
  return env.GITHUB_ACTIONS === 'true' || env.TAURI_BUNDLE === '1';
}

function shouldUseFastLocalBuild(args, env = process.env) {
  if (!isBuildCommand(args) || hasDebugFlag(args)) {
    return false;
  }

  if (hasBundleFlag(args) || shouldBundle(env)) {
    return false;
  }

  return env[LOCAL_FAST_BUILD_DISABLE_ENV] !== '1';
}

function getArgValue(args, longFlag, shortFlag) {
  const longIndex = args.indexOf(longFlag);
  if (longIndex >= 0) {
    return args[longIndex + 1];
  }

  const shortIndex = args.indexOf(shortFlag);
  if (shortIndex >= 0) {
    return args[shortIndex + 1];
  }

  return null;
}

function getTargetTriple(args, env = process.env) {
  return (
    getArgValue(args, '--target', '-t') ||
    env.CARGO_BUILD_TARGET ||
    env.TAURI_BUILD_TARGET ||
    DEFAULT_WINDOWS_TARGET
  );
}

function getTargetLinkerEnvKey(targetTriple) {
  return `CARGO_TARGET_${targetTriple.toUpperCase().replace(/-/g, '_')}_LINKER`;
}

function resolveRustSysroot() {
  try {
    return execFileSync('rustc', ['--print', 'sysroot'], { encoding: 'utf8' }).trim();
  } catch {
    return null;
  }
}

function resolveRustLldPath(targetTriple, options = {}) {
  const sysroot = options.sysroot || resolveRustSysroot();
  if (!sysroot) {
    return null;
  }

  const executableName = process.platform === 'win32' ? 'rust-lld.exe' : 'rust-lld';
  const lldPath = path.join(sysroot, 'lib', 'rustlib', targetTriple, 'bin', executableName);
  const pathExists = options.pathExists || existsSync;
  return pathExists(lldPath) ? lldPath : null;
}

export function buildTauriCliArgs(args, env = process.env) {
  if (!isBuildCommand(args)) {
    return args;
  }

  if (hasBundleFlag(args) || shouldBundle(env)) {
    return args;
  }

  return [...args, '--no-bundle'];
}

export function buildTauriCliEnv(args, env = process.env, options = {}) {
  if (!shouldUseFastLocalBuild(args, env)) {
    return env;
  }

  const nextEnv = { ...env };
  nextEnv.CARGO_PROFILE_RELEASE_INCREMENTAL ??= 'true';
  nextEnv.CARGO_PROFILE_RELEASE_CODEGEN_UNITS ??= '256';
  nextEnv.CARGO_PROFILE_RELEASE_OPT_LEVEL ??= '2';
  nextEnv.CARGO_PROFILE_RELEASE_DEBUG ??= '0';

  const targetTriple = options.targetTriple || getTargetTriple(args, env);
  const linkerEnvKey = getTargetLinkerEnvKey(targetTriple);
  if (!nextEnv[linkerEnvKey]) {
    const rustLldPath = options.rustLldPath || resolveRustLldPath(targetTriple, options);
    if (rustLldPath) {
      nextEnv[linkerEnvKey] = rustLldPath;
    }
  }

  return nextEnv;
}

export function describeLocalBuildOptimization(args, env = process.env, options = {}) {
  if (!shouldUseFastLocalBuild(args, env)) {
    return null;
  }

  const targetTriple = options.targetTriple || getTargetTriple(args, env);
  const linkerEnvKey = getTargetLinkerEnvKey(targetTriple);
  const tauriEnv = buildTauriCliEnv(args, env, options);
  const details = [
    'release incremental',
    `codegen-units=${tauriEnv.CARGO_PROFILE_RELEASE_CODEGEN_UNITS}`,
    `opt-level=${tauriEnv.CARGO_PROFILE_RELEASE_OPT_LEVEL}`,
  ];

  if (tauriEnv[linkerEnvKey]) {
    details.push('rust-lld');
  }

  return `本地 tauri build 已启用快速构建配置：${details.join(' / ')}`;
}
