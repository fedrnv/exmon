// Run only in a disposable checkout; each race retry starts from accepted remote main.
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { allocate } from './fdtasks-version-allocator.mjs';

export function publish(root, { beforePush = () => {}, attempts = 5 } = {}) {
  const git = (...args) => {
    const result = spawnSync('git', args, { cwd: root, encoding: 'utf8', timeout: 60000, env: { ...process.env, GIT_TERMINAL_PROMPT: '0', GCM_INTERACTIVE: 'never' } });
    if (result.error || result.status !== 0) throw new Error(`git ${args.join(' ')}: ${result.error?.message ?? result.stderr}`);
    return result.stdout.trim();
  };
  if (git('status', '--porcelain')) throw new Error('Publisher requires a clean disposable checkout');
  for (let attempt = 1; attempt <= attempts; attempt++) {
    git('fetch', 'origin', 'main');
    git('reset', '--hard', 'origin/main');
    allocate(root);
    git('add', 'Tasks/version.json', 'Tasks/version-events', 'Tasks/VERSION_HISTORY.md');
    if (!git('diff', '--cached', '--name-only')) return;
    git('commit', '-m', 'chore(version): allocate FDTasks versions [skip version]');
    beforePush(attempt);
    try { git('push', 'origin', 'HEAD:main'); return; }
    catch (error) { if (attempt === attempts) throw error; }
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.env.GITHUB_ACTIONS !== 'true') throw new Error('Publisher requires GitHub Actions');
  publish(process.cwd());
}
