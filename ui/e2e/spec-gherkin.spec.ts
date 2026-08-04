import { test, expect } from './fixtures';

test('spec page renders Gherkin scenarios as highlighted code', async ({ page }) => {
	await page.goto('/specifications/spec-001');
	const gherkin = page.locator('code.language-gherkin').first();
	await expect(gherkin).toBeVisible();
	await expect(gherkin).toContainText('Scenario:');
});

test('spec page renders a scenario-flow diagram generated from Gherkin', async ({ page }) => {
	await page.goto('/specifications/spec-001');
	await expect(page.getByText('Scenario flow')).toBeVisible();
	await expect(page.locator('svg[id^="mermaid-"]').first()).toBeVisible({ timeout: 20_000 });
});

test('doc page shows a mini-graph of related documents', async ({ page }) => {
	await page.goto('/specifications/spec-001');
	const miniGraph = page.getByTestId('mini-graph');
	await expect(miniGraph).toBeVisible();
	await expect(miniGraph.locator('.svelte-flow__node-doc').first()).toBeVisible({
		timeout: 15_000
	});
	// Focus deep link into the full graph
	await page.getByRole('link', { name: /View in full graph/ }).click();
	await expect(page).toHaveURL(/\/graph\?focus=spec-001/);
	await expect(page.getByRole('button', { name: /Focused on SPEC-001/ })).toBeVisible();
});

test('relation sidebar items show status badges', async ({ page }) => {
	await page.goto('/specifications/spec-001');
	// OPP-001 (implements target) is accepted/pursuing — a badge should render in its sidebar item
	const sidebarItem = page.locator('a[href*="/opportunities/opp-001"]').first();
	await expect(sidebarItem).toBeVisible();
	await expect(sidebarItem.locator('[class*="badge"], .text-\\[9px\\]').first()).toBeVisible();
});

test('onboarding page lists active opportunities as Start here', async ({ page }) => {
	await page.goto('/onboarding');
	await expect(page.getByTestId('start-here')).toBeVisible();
	await expect(page.getByTestId('start-here').locator('a').first()).toBeVisible();
});

test('relation sidebar links navigate to the target doc', async ({ page }) => {
	await page.goto('/specifications/spec-001');
	// spec-001 implements OPP-001 and depends on ADR-001
	const relLink = page.locator('a[href*="/opportunities/opp-001"]').first();
	await expect(relLink).toBeVisible();
	await relLink.click();
	await expect(page).toHaveURL(/\/opportunities\/opp-001/);
	await expect(page.locator('h1').first()).toBeVisible();
});
