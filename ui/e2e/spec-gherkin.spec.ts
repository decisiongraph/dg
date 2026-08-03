import { test, expect } from './fixtures';

test('spec page renders Gherkin scenarios as highlighted code', async ({ page }) => {
	await page.goto('/specifications/spec-001');
	const gherkin = page.locator('code.language-gherkin').first();
	await expect(gherkin).toBeVisible();
	await expect(gherkin).toContainText('Scenario:');
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
