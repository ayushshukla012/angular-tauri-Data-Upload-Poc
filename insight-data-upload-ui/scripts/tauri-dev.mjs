import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const projectRoot = fileURLToPath(new URL('..', import.meta.url));
const configPath = join(projectRoot, 'src/assets/config/app-config.json');
const appConfig = JSON.parse(await import('node:fs/promises').then(fs => fs.readFile(configPath, 'utf8')));
const host = appConfig.tooling?.devUrlHost;
const port = appConfig.tooling?.devServerPort;
if (!host || !port) throw new Error('tooling.devUrlHost and tooling.devServerPort are required.');

const tempDir = mkdtempSync(join(tmpdir(), 'insight-tauri-dev-'));
const tempConfig = join(tempDir, 'tauri.dev.json');
writeFileSync(tempConfig, JSON.stringify({ build: { devUrl: `${host.replace(/\/$/, '')}:${port}` } }, null, 2));

const cli = join(projectRoot, 'node_modules/@tauri-apps/cli/tauri.js');
const child = spawn(process.execPath, [cli, 'dev', '--config', tempConfig], { cwd: projectRoot, stdio: 'inherit' });
const cleanup = () => { try { rmSync(tempDir, { recursive: true, force: true }); } catch {} };
process.on('exit', cleanup);
process.on('SIGINT', () => child.kill('SIGINT'));
process.on('SIGTERM', () => child.kill('SIGTERM'));
child.on('exit', (code, signal) => { cleanup(); process.exit(signal ? 1 : (code ?? 1)); });
