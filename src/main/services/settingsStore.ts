import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import {
  appSettingsPatchSchema,
  appSettingsSchema,
  defaultAppSettings,
  type AppSettings,
  type AppSettingsPatch
} from '../../shared/contracts'

export class SettingsStore {
  private readonly settingsPath: string

  constructor(userDataPath: string) {
    this.settingsPath = join(userDataPath, 'settings.json')
  }

  getSettings(): AppSettings {
    if (!existsSync(this.settingsPath)) return cloneSettings(defaultAppSettings)

    try {
      return appSettingsSchema.parse(JSON.parse(readFileSync(this.settingsPath, 'utf8')))
    } catch {
      return cloneSettings(defaultAppSettings)
    }
  }

  updateSettings(input: AppSettingsPatch): AppSettings {
    const patch = appSettingsPatchSchema.parse(input)
    const previous = this.getSettings()
    const next = appSettingsSchema.parse({
      ...previous,
      anonymization: {
        ...previous.anonymization,
        ...patch.anonymization
      },
      files: {
        ...previous.files,
        ...patch.files
      }
    })

    this.writeSettings(next)
    return next
  }

  private writeSettings(settings: AppSettings): void {
    mkdirSync(dirname(this.settingsPath), { recursive: true })
    const tempPath = `${this.settingsPath}.tmp`
    writeFileSync(tempPath, `${JSON.stringify(settings, null, 2)}\n`)
    renameSync(tempPath, this.settingsPath)
  }
}

function cloneSettings(settings: AppSettings): AppSettings {
  return appSettingsSchema.parse(JSON.parse(JSON.stringify(settings)))
}
