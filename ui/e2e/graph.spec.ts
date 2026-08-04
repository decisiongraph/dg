import { test, expect } from './fixtures';

test('graph renders doc nodes and status chips filter them', async ({ page, request }) => {
	const graph = await (await request.get('/data/graph.json')).json();
	const nodes = graph.nodes as { id: string; status?: string }[];
	expect(nodes.length).toBeGreaterThan(0);

	await page.goto('/graph');
	// Group container nodes also use .svelte-flow__node, so target doc nodes only.
	// The page applies its own visibility rules (hidden statuses, unconnected
	// nodes), so assert bounds rather than an exact prediction.
	const docNodes = page.locator('.svelte-flow__node-doc');
	await expect(docNodes.first()).toBeVisible({ timeout: 20_000 });
	const rendered = await docNodes.count();
	expect(rendered).toBeGreaterThan(0);
	expect(rendered).toBeLessThanOrEqual(nodes.length);

	// Toggling off the most common status must reduce the rendered count
	const statusCounts = new Map<string, number>();
	for (const n of nodes) {
		const s = n.status?.toLowerCase();
		if (s) statusCounts.set(s, (statusCounts.get(s) ?? 0) + 1);
	}
	const topStatus = [...statusCounts.entries()].sort((a, b) => b[1] - a[1])[0][0];
	await page.getByRole('button', { name: topStatus }).first().click();
	await expect
		.poll(async () => docNodes.count(), { timeout: 10_000 })
		.toBeLessThan(rendered);
});

test('clicking a graph node navigates to the doc page', async ({ page }) => {
	await page.goto('/graph');
	const node = page.locator('.svelte-flow__node-doc').first();
	await expect(node).toBeVisible({ timeout: 15_000 });
	await node.click();
	await expect(page).toHaveURL(
		/\/(architecture|policies|opportunities|specifications|incidents|processes)\//,
		{ timeout: 10_000 }
	);
});
