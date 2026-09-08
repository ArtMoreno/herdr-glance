// End-to-end process AND captured-pipe latency, using only the fake Herdr CLI.
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const root = path.resolve(__dirname, '..');
const dir = path.join(root, 'target', 'line-bench-' + process.pid);
fs.mkdirSync(dir, { recursive: true });
const suffix = process.platform === 'win32' ? '.exe' : '';
const fake = path.join(dir, 'fake-herdr' + suffix);
const build = spawnSync('rustc', ['--edition=2021', path.join(root, 'tests/support/fake-herdr.rs'), '-o', fake], { stdio: 'inherit' });
if (build.status !== 0) process.exit(1);
const binary = path.join(root, 'target/release/glance' + suffix);
const env = { ...process.env, HERDR_ENV: '1', HERDR_WORKSPACE_ID: 'w-demo', HERDR_SESSION: 'fixture', HERDR_GLANCE_BIN: fake, HERDR_GLANCE_CACHE_DIR: path.join(dir, 'cache'), XDG_CONFIG_HOME: dir, APPDATA: dir };
function run(args) {
  const start = performance.now();
  const r = spawnSync(binary, args, { env, encoding: 'utf8', timeout: 5000 });
  const ms = performance.now() - start;
  if (r.status !== 0) throw new Error(r.stderr || r.error || 'nonzero exit');
  return ms;
}
run(['--refresh']);
const warm = Array.from({ length: 30 }, () => run(['line', '--plain']));
const cache = path.join(dir, 'cache', fs.readdirSync(path.join(dir, 'cache')).find(x => x.endsWith('.json')));
const value = JSON.parse(fs.readFileSync(cache));
value.at = 0;
fs.writeFileSync(cache, JSON.stringify(value));
fs.writeFileSync(path.join(dir, 'slow'), '');
const cold = Array.from({ length: 10 }, () => run(['line', '--plain']));
const stats = values => {
  values.sort((a, b) => a - b);
  return { n: values.length, median_ms: +values[Math.floor(values.length / 2)].toFixed(2), p95_ms: +values[Math.ceil(values.length * .95) - 1].toFixed(2), max_ms: +values.at(-1).toFixed(2) };
};
const result = { at: new Date().toISOString(), platform: process.platform, mode: 'release; fake Herdr; includes captured pipe EOF', warm: stats(warm), expired_with_slow_backend: stats(cold) };
fs.writeFileSync(path.join(root, 'docs', 'line-benchmark.json'), JSON.stringify(result, null, 2) + '\n');
console.log(JSON.stringify(result, null, 2));
