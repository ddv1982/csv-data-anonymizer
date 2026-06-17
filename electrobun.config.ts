import { readFileSync } from 'node:fs'
import type { ElectrobunConfig } from 'electrobun/bun'

const packageJson = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8')) as {
  version: string
  description?: string
}
const macCodesignEnabled = process.env.ELECTROBUN_CODESIGN === '1' || Boolean(process.env.ELECTROBUN_DEVELOPER_ID)
const macNotarizeEnabled =
  process.env.ELECTROBUN_NOTARIZE === '1' ||
  Boolean(
    process.env.ELECTROBUN_APPLEAPIISSUER &&
      process.env.ELECTROBUN_APPLEAPIKEY &&
      process.env.ELECTROBUN_APPLEAPIKEYPATH
  ) ||
  Boolean(process.env.ELECTROBUN_APPLEID && process.env.ELECTROBUN_APPLEIDPASS && process.env.ELECTROBUN_TEAMID)

export default {
  app: {
    name: 'CSV Anonymizer',
    identifier: 'com.csv-anonymizer.app',
    version: packageJson.version,
    description: packageJson.description
  },
  build: {
    bun: {
      entrypoint: 'src/bun/index.ts'
    },
    copy: {
      'dist/renderer': 'views/mainview'
    },
    buildFolder: 'dist/electrobun/build',
    artifactFolder: 'dist/electrobun/artifacts',
    watch: ['src/bun', 'src/services', 'src/shared', 'src/core', 'src/strategies', 'src/types', 'src/utils', 'dist/renderer'],
    watchIgnore: ['dist/electrobun/**'],
    mac: {
      bundleCEF: false,
      defaultRenderer: 'native',
      icons: 'build/macos/AppIcon.iconset',
      codesign: macCodesignEnabled,
      notarize: macNotarizeEnabled
    },
    linux: {
      bundleCEF: true,
      defaultRenderer: 'cef',
      icon: 'build/icons/1024x1024.png'
    },
    win: {
      defaultRenderer: 'native',
      icon: 'build/icons/1024x1024.png'
    }
  },
  runtime: {
    exitOnLastWindowClosed: true
  },
  release: {
    baseUrl: process.env.ELECTROBUN_RELEASE_BASE_URL,
    generatePatch: false
  }
} satisfies ElectrobunConfig
