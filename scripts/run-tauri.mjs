import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  buildTauriCliArgs,
  buildTauriCliEnv,
  describeLocalBuildOptimization,
} from './tauriCliUtils.mjs';

const userArgs = process.argv.slice(2);
const tauriArgs = buildTauriCliArgs(userArgs);
const tauriEnv = buildTauriCliEnv(userArgs, process.env);
const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const tauriCliEntry = path.resolve(
  scriptDir,
  '..',
  'node_modules',
  '@tauri-apps',
  'cli',
  'tauri.js'
);

const optimizationMessage = describeLocalBuildOptimization(userArgs, process.env);
if (optimizationMessage) {
  console.log(optimizationMessage);
}

const child = spawn(process.execPath, [tauriCliEntry, ...tauriArgs], {
  stdio: 'inherit',
  env: tauriEnv,
});

child.on('exit', (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }

  process.exit(code ?? 1);
});

child.on('error', (error) => {
  console.error('启动 tauri CLI 失败:', error);
  process.exit(1);
});
