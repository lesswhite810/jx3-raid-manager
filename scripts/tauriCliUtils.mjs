export function buildTauriCliArgs(args, env = process.env) {
  if (args[0] !== 'build') {
    return args;
  }

  const hasBundleFlag = args.includes('--bundles') || args.includes('--no-bundle');
  const shouldBundle = env.GITHUB_ACTIONS === 'true' || env.TAURI_BUNDLE === '1';

  if (hasBundleFlag || shouldBundle) {
    return args;
  }

  return [...args, '--no-bundle'];
}
