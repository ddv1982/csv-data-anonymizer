declare module 'electrobun/bun' {
  export interface Rectangle {
    x: number
    y: number
    width: number
    height: number
  }

  export interface Display {
    id: number
    bounds: Rectangle
    workArea: Rectangle
    scaleFactor: number
    isPrimary: boolean
  }

  export interface PlatformBuildConfig {
    bundleCEF?: boolean
    bundleWGPU?: boolean
    defaultRenderer?: 'native' | 'cef'
    chromiumFlags?: Record<string, string | boolean>
  }

  export interface ElectrobunConfig {
    app: {
      name: string
      identifier: string
      version: string
      description?: string
    }
    build?: {
      bun?: { entrypoint?: string; [key: string]: unknown }
      views?: Record<string, { entrypoint: string; [key: string]: unknown }>
      copy?: Record<string, string>
      buildFolder?: string
      artifactFolder?: string
      watch?: string[]
      watchIgnore?: string[]
      mac?: PlatformBuildConfig & { codesign?: boolean; notarize?: boolean; createDmg?: boolean; icons?: string }
      linux?: PlatformBuildConfig & { icon?: string }
      win?: PlatformBuildConfig & { icon?: string }
    }
    runtime?: {
      exitOnLastWindowClosed?: boolean
      [key: string]: unknown
    }
    release?: {
      baseUrl?: string
      generatePatch?: boolean
    }
  }

  export interface BrowserWindowOptions<T = unknown> {
    title?: string
    frame?: Rectangle
    url?: string | null
    html?: string | null
    preload?: string | null
    viewsRoot?: string | null
    renderer?: 'native' | 'cef'
    rpc?: T
    titleBarStyle?: 'hidden' | 'hiddenInset' | 'default'
    transparent?: boolean
    passthrough?: boolean
    hidden?: boolean
    navigationRules?: string | null
    sandbox?: boolean
  }

  export class BrowserWindow<T = unknown> {
    constructor(options?: BrowserWindowOptions<T>)
    readonly id: number
    readonly webview: BrowserView<T>
    show(): unknown
    close(): unknown
    activate(): unknown
    hide(): unknown
    minimize(): unknown
    unminimize(): unknown
    isMinimized(): boolean
    setFullScreen(fullScreen: boolean): unknown
    isFullScreen(): boolean
    setFrame(x: number, y: number, width: number, height: number): unknown
    getFrame(): Rectangle
    on(name: string, handler: (event: unknown) => void): void
  }

  export class BrowserView<T = unknown> {
    static defineRPC<Schema>(config: unknown): Schema
    on(name: string, handler: (event: unknown) => void): void
    loadURL(url: string): unknown
  }

  export const Screen: {
    getPrimaryDisplay(): Display
    getAllDisplays(): Display[]
  }

  export const Utils: {
    paths: {
      home: string
      appData: string
      config: string
      cache: string
      temp: string
      logs: string
      documents: string
      downloads: string
      desktop: string
      pictures: string
      music: string
      videos: string
      userData: string
      userCache: string
      userLogs: string
    }
    openFileDialog(options?: {
      startingFolder?: string
      allowedFileTypes?: string
      canChooseFiles?: boolean
      canChooseDirectory?: boolean
      allowsMultipleSelection?: boolean
    }): Promise<string[]>
    showItemInFolder(path: string): unknown
    openExternal(url: string): boolean
    quit(): void
  }

  export const Updater: {
    localInfo: {
      version(): Promise<string>
    }
  }
}
