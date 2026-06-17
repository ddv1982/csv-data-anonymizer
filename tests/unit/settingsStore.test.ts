import { mkdtemp, rm } from 'node:fs/promises';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { defaultAppSettings } from '../../src/shared/contracts';
import { SettingsStore } from '../../src/main/services/settingsStore';

describe('SettingsStore', () => {
  let tempDir: string;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'csv-anonymizer-settings-'));
  });

  afterEach(async () => {
    await rm(tempDir, { recursive: true, force: true });
  });

  it('returns default settings when no settings file exists', () => {
    const store = new SettingsStore(tempDir);

    expect(store.getSettings()).toEqual(defaultAppSettings);
  });

  it('persists app settings updates as versioned JSON', () => {
    const store = new SettingsStore(tempDir);

    const updated = store.updateSettings({
      anonymization: {
        deterministicDefault: true,
        seed: 'team-seed',
        overwriteOutput: false,
      },
    });

    expect(updated.anonymization.deterministicDefault).toBe(true);
    expect(updated.anonymization.seed).toBe('team-seed');
    expect(updated.anonymization.overwriteOutput).toBe(false);
    expect(updated.files).toEqual(defaultAppSettings.files);
    expect(existsSync(join(tempDir, 'settings.json'))).toBe(true);

    const reopened = new SettingsStore(tempDir);
    expect(reopened.getSettings()).toEqual(updated);
  });

  it('falls back to defaults when the settings file is invalid', () => {
    const store = new SettingsStore(tempDir);
    store.updateSettings({ anonymization: { seed: 'valid' } });
    const settingsPath = join(tempDir, 'settings.json');
    expect(readFileSync(settingsPath, 'utf8')).toContain('valid');

    writeFileSync(settingsPath, '{');
    const recovered = new SettingsStore(tempDir);

    expect(recovered.getSettings()).toEqual(defaultAppSettings);
  });
});
