import { test, expect, startDgServer, type DgServer } from './fixtures';
import fs from 'node:fs';
import path from 'node:path';

// Runs against its OWN dg serve instance: the rebuild it triggers rewrites the
// whole site output, which would race pages other tests are loading if it
// shared the worker-scoped server.
let server: DgServer;

test.beforeAll(async () => {
	server = await startDgServer(18900 + test.info().workerIndex * 5);
});

test.afterAll(() => {
	server?.stop();
});

test('editing a doc triggers a rebuild that reaches the served data', async ({ page }) => {
	test.setTimeout(90_000);

	const marker = `live-reload-check-${test.info().workerIndex}`;
	const docPath = path.join(server.dir, 'docs/architecture/adr-001.md');
	fs.appendFileSync(docPath, `\n\nRebuild sentinel: ${marker}\n`);

	// Poll the generated data until the watcher+rebuild pipeline picks up the change
	await expect
		.poll(
			async () => {
				const res = await fetch(`${server.url}/data/docs.json`);
				if (!res.ok) return false;
				return (await res.text()).includes(marker);
			},
			{ timeout: 45_000, intervals: [1000] }
		)
		.toBe(true);

	// And the SPA shows it after a reload (no client push-reload by design)
	await page.goto(`${server.url}/architecture/adr-001`);
	await expect(page.getByText(marker)).toBeVisible({ timeout: 10_000 });
});
