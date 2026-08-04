/**
 * Playwright fixtures: each worker gets its own `dg serve` instance running
 * against a private temp copy of the bundled example/ project. The live-reload
 * spec uses a dedicated per-test server (freshServer) because its rebuilds
 * rewrite the site output while other tests would be loading pages from it.
 *
 * Requires a prebuilt debug binary: `cargo build -p dg-cli` (run `bun run build`
 * first so the embedded SPA is current).
 */
import { test as base, expect } from '@playwright/test';
import { spawn, type ChildProcess } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '../..');
const dgBinary = path.join(
	repoRoot,
	'target',
	process.env.DG_E2E_PROFILE ?? 'debug',
	process.platform === 'win32' ? 'dg.exe' : 'dg'
);

export interface DgServer {
	url: string;
	/** Root of the temp copy of example/ this server is serving. */
	dir: string;
	stop: () => void;
}

async function waitForServer(url: string, timeoutMs = 30_000): Promise<void> {
	const deadline = Date.now() + timeoutMs;
	let lastError: unknown;
	while (Date.now() < deadline) {
		try {
			const res = await fetch(url);
			if (res.ok) return;
			lastError = new Error(`HTTP ${res.status}`);
		} catch (err) {
			lastError = err;
		}
		await new Promise((r) => setTimeout(r, 200));
	}
	throw new Error(`dg serve at ${url} did not become ready: ${lastError}`);
}

export async function startDgServer(requestedPort: number): Promise<DgServer> {
	if (!fs.existsSync(dgBinary)) {
		throw new Error(
			`dg binary not found at ${dgBinary}. Run: cd ui && bun run build && cd .. && cargo build -p dg-cli`
		);
	}

	const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'dg-e2e-'));
	fs.cpSync(path.join(repoRoot, 'example'), dir, { recursive: true });

	// Share .dg/cache (the ~8MB d2 browser bundle + avatar lookups) across
	// workers/runs so only the first server ever hits the network for them.
	const sharedCache = path.join(os.tmpdir(), 'dg-e2e-shared-cache');
	const tempCache = path.join(dir, '.dg', 'cache');
	if (fs.existsSync(sharedCache)) {
		fs.cpSync(sharedCache, tempCache, { recursive: true });
	}

	// dg auto-increments if the port is busy and prints the actual one
	const child: ChildProcess = spawn(
		dgBinary,
		['serve', '--port', String(requestedPort), '--host', '127.0.0.1'],
		{ cwd: dir, stdio: ['ignore', 'pipe', 'pipe'] }
	);

	let stderrBuf = '';
	child.stderr?.on('data', (chunk) => (stderrBuf += chunk));

	const url = await new Promise<string>((resolve, reject) => {
		let stdoutBuf = '';
		const timer = setTimeout(
			() => reject(new Error(`dg serve produced no address. stderr:\n${stderrBuf}`)),
			60_000
		);
		child.stdout?.on('data', (chunk) => {
			stdoutBuf += chunk;
			const match = stdoutBuf.match(/Serving at http:\/\/([\d.]+:\d+)/);
			if (match) {
				clearTimeout(timer);
				resolve(`http://${match[1]}`);
			}
		});
		child.on('exit', (code) => {
			clearTimeout(timer);
			reject(new Error(`dg serve exited with code ${code}. stderr:\n${stderrBuf}`));
		});
	});

	await waitForServer(`${url}/data/docs.json`);

	// Populate the shared cache from whichever server fetched it first.
	// Stage + rename so a concurrently-starting server never sees a
	// half-written cache directory.
	if (!fs.existsSync(sharedCache) && fs.existsSync(tempCache)) {
		try {
			const staging = `${sharedCache}.${process.pid}.tmp`;
			fs.cpSync(tempCache, staging, { recursive: true });
			fs.renameSync(staging, sharedCache);
		} catch {
			// another worker won the race — fine
			fs.rmSync(`${sharedCache}.${process.pid}.tmp`, { recursive: true, force: true });
		}
	}

	return {
		url,
		dir,
		stop: () => {
			child.kill('SIGTERM');
			fs.rmSync(dir, { recursive: true, force: true });
		}
	};
}

export const test = base.extend<Record<string, never>, { dgServer: DgServer }>({
	dgServer: [
		// eslint-disable-next-line no-empty-pattern
		async ({}, use, workerInfo) => {
			const server = await startDgServer(18300 + workerInfo.workerIndex * 20);
			await use(server);
			server.stop();
		},
		{ scope: 'worker' }
	],

	baseURL: async ({ dgServer }, use) => {
		await use(dgServer.url);
	}
});

export { expect };
