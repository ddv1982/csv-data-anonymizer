import { _electron as electron, expect, test } from '@playwright/test';

test('opens the desktop anonymizer shell', async () => {
  const app = await electron.launch({ args: ['.'] });
  const page = await app.firstWindow();

  await expect(page.getByRole('heading', { name: 'CSV Anonymizer' })).toBeVisible();

  await app.close();
});
