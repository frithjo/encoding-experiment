# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: batch_dla_scan_ui.spec.ts >> Batch DLA Scan UI >> results visualization - attention matrices section appears
- Location: tests/batch_dla_scan_ui.spec.ts:50:7

# Error details

```
Test timeout of 30000ms exceeded.
```

```
Error: locator.fill: Test timeout of 30000ms exceeded.
Call log:
  - waiting for locator('textarea[placeholder="Enter text to analyze..."]')

```

# Test source

```ts
  1   | import { test, expect } from '@playwright/test';
  2   | 
  3   | test.describe('Batch DLA Scan UI', () => {
  4   |   test.beforeEach(async ({ page }) => {
  5   |     await page.goto('/');
  6   |   });
  7   | 
  8   |   test('input interaction - textarea accepts text', async ({ page }) => {
  9   |     const textarea = page.locator('textarea[placeholder="Enter text to analyze..."]');
  10  |     await expect(textarea).toBeVisible();
  11  |     await textarea.fill('test prompt');
  12  |     await expect(textarea).toHaveValue('test prompt');
  13  |   });
  14  | 
  15  |   test('button state - enabled when prompt is non-empty', async ({ page }) => {
  16  |     const button = page.locator('button');
  17  |     const textarea = page.locator('textarea[placeholder="Enter text to analyze..."]');
  18  |     
  19  |     // Initially disabled when empty
  20  |     await expect(button).toBeDisabled();
  21  |     
  22  |     // Enabled after entering text
  23  |     await textarea.fill('test prompt');
  24  |     await expect(button).toBeEnabled();
  25  |   });
  26  | 
  27  |   test('button state - shows Scanning during load', async ({ page }) => {
  28  |     const button = page.locator('button');
  29  |     const textarea = page.locator('textarea[placeholder="Enter text to analyze..."]');
  30  |     
  31  |     await textarea.fill('test prompt');
  32  |     
  33  |     // Start scan and check button text changes
  34  |     const scanPromise = button.click();
  35  |     await expect(button).toHaveText('Scanning...');
  36  |     
  37  |     // Wait for scan to complete (or timeout)
  38  |     await scanPromise.catch(() => {});
  39  |   });
  40  | 
  41  |   test('error display - empty prompt error', async ({ page }) => {
  42  |     const button = page.locator('button');
  43  |     const errorDiv = page.locator('.error');
  44  |     
  45  |     await button.click();
  46  |     await expect(errorDiv).toBeVisible();
  47  |     await expect(errorDiv).toContainText('Please enter a prompt');
  48  |   });
  49  | 
  50  |   test('results visualization - attention matrices section appears', async ({ page }) => {
  51  |     const textarea = page.locator('textarea[placeholder="Enter text to analyze..."]');
  52  |     const button = page.locator('button');
  53  |     const resultsSection = page.locator('.results');
  54  |     
> 55  |     await textarea.fill('test prompt');
      |                    ^ Error: locator.fill: Test timeout of 30000ms exceeded.
  56  |     await button.click();
  57  |     
  58  |     // Wait for results (with timeout since server might not be running)
  59  |     try {
  60  |       await expect(resultsSection).toBeVisible({ timeout: 5000 });
  61  |       await expect(resultsSection).toContainText('Attention Matrices');
  62  |     } catch (e) {
  63  |       // If server is not running, skip this test
  64  |       test.skip();
  65  |     }
  66  |   });
  67  | 
  68  |   test('data accuracy - layer cards show correct information', async ({ page }) => {
  69  |     const textarea = page.locator('textarea[placeholder="Enter text to analyze..."]');
  70  |     const button = page.locator('button');
  71  |     const layerCards = page.locator('.layer-card');
  72  |     
  73  |     await textarea.fill('test prompt');
  74  |     await button.click();
  75  |     
  76  |     try {
  77  |       await expect(layerCards.first()).toBeVisible({ timeout: 5000 });
  78  |       
  79  |       const firstCard = layerCards.first();
  80  |       await expect(firstCard.locator('h3')).toContainText('Layer');
  81  |       await expect(firstCard).toContainText('Heads:');
  82  |       await expect(firstCard).toContainText('Positions:');
  83  |     } catch (e) {
  84  |       test.skip();
  85  |     }
  86  |   });
  87  | 
  88  |   test('stats display - shows layer and token counts', async ({ page }) => {
  89  |     const textarea = page.locator('textarea[placeholder="Enter text to analyze..."]');
  90  |     const button = page.locator('button');
  91  |     const stats = page.locator('.stats');
  92  |     
  93  |     await textarea.fill('test prompt');
  94  |     await button.click();
  95  |     
  96  |     try {
  97  |       await expect(stats).toBeVisible({ timeout: 5000 });
  98  |       await expect(stats).toContainText('Layers:');
  99  |       await expect(stats).toContainText('Tokens:');
  100 |     } catch (e) {
  101 |       test.skip();
  102 |     }
  103 |   });
  104 | });
  105 | 
```