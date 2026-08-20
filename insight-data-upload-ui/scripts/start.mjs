import { readFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

const projectRoot = fileURLToPath(new URL('..', import.meta.url));
const config = JSON.parse(await readFile(join(projectRoot, 'src/assets/config/app-config.json'), 'utf8'));
const host = config.tooling?.devServerHost;
const port = Number(config.tooling?.devServerPort);
if (!host || !Number.isFinite(port)) throw new Error('Runtime tooling configuration is missing devServerHost/devServerPort.');

const cli = join(projectRoot, 'node_modules/@angular/cli/bin/ng.js');
const child = spawn(process.execPath, [cli, 'serve', '--host', host, '--port', String(port)], { cwd: projectRoot, stdio: 'inherit' });
child.on('exit', code => process.exit(code ?? 0));
