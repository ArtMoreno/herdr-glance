// Read-only live CLI checks; inherits the genuine Herdr pane environment.
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
assert.equal(process.env.HERDR_ENV, '1');
assert.ok(process.env.HERDR_WORKSPACE_ID);
const root = path.resolve(__dirname, '..');
const dir = path.join(root, 'target', 'live-bench-' + process.pid);
fs.mkdirSync(dir, { recursive: true });
const binary = path.join(root, 'target/release/glance' + (process.platform === 'win32' ? '.exe' : ''));
const env = { ...process.env, HERDR_GLANCE_CACHE_DIR: dir };
function run(args) {
  const start = performance.now();
  const r = spawnSync(binary, args, { env, encoding: 'utf8', timeout: 5000 });
  assert.equal(r.status, 0, r.stderr || String(r.error));
  return { ms: performance.now() - start, text: r.stdout.trim() };
}
function cache(scope) {
  for (const name of fs.readdirSync(dir).filter(n => n.endsWith('.json'))) {
    const file = path.join(dir, name);
    const data = JSON.parse(fs.readFileSync(file));
    if (data.workspace === scope) return { file, data };
  }
}
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
(async () => {
  const refresh = run(['--refresh', '--all-workspaces']).ms;
  run(['--refresh']);
  assert.notEqual(cache('*').file, cache(env.HERDR_WORKSPACE_ID).file);
  const plain = run(['line', '--all-workspaces', '--plain']);
  assert.ok(!plain.text.includes('\x1b'));
  const words = run(['line', '--all-workspaces', '--plain', '--no-emoji']);
  assert.match(words.text, /(?:working|idle|blocked|done|unknown):\d/);
  assert.ok(run(['line', '--all-workspaces']).text.includes('\x1b'));
  const warm = Array.from({ length: 30 }, () => run(['line', '--all-workspaces', '--plain']).ms);
  const expired = [];
  for (let i = 0; i < 10; i++) {
    const { file, data } = cache('*');
    data.at = 0;
    fs.writeFileSync(file, JSON.stringify(data));
    const r = run(['line', '--all-workspaces', '--plain', '--no-emoji']);
    assert.equal(r.text, 'unknown');
    expired.push(r.ms);
    const deadline = Date.now() + 5000;
    while ((!fs.existsSync(file) || cache('*')?.data.at === 0) && Date.now() < deadline) await sleep(25);
    assert.ok(cache('*').data.at > 0, 'detached refresh lost session scope');
  }
  const stats = values => {
    values.sort((a, b) => a - b);
    return { n: values.length, median_ms: +values[Math.floor(values.length / 2)].toFixed(2), max_ms: +values.at(-1).toFixed(2) };
  };
  const result = { at: new Date().toISOString(), platform: process.platform, mode: 'release; live Herdr; includes process startup and captured-pipe EOF', refresh_ms: +refresh.toFixed(2), warm: stats(warm), expired: stats(expired), checks: ['plain', 'ANSI', 'word badges', 'workspace/session cache separation', 'expired returns unknown', 'detached refresh retains session scope'], counts: cache('*').data.counts };
  fs.writeFileSync(path.join(root, 'docs/live-benchmark.json'), JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify(result, null, 2));
})().catch(e => { console.error(e); process.exitCode = 1; });
