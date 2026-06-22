import { useEffect, useState } from 'react'
import { setAppTheme } from '../tauri'
import type { ThemeMode } from '../types'

export type { ThemeMode } from '../types'
export type ResolvedTheme = 'light' | 'dark'

const systemThemeQuery = '(prefers-color-scheme: dark)'

export function useTheme(themeMode: ThemeMode) {
  const [systemTheme, setSystemTheme] = useState<ResolvedTheme>(() => getSystemTheme())
  const resolvedTheme = themeMode === 'system' ? systemTheme : themeMode

  useEffect(() => {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return

    const media = window.matchMedia(systemThemeQuery)
    const handleChange = () => setSystemTheme(media.matches ? 'dark' : 'light')

    handleChange()
    if (typeof media.addEventListener === 'function') {
      media.addEventListener('change', handleChange)
      return () => media.removeEventListener('change', handleChange)
    }

    media.addListener(handleChange)
    return () => media.removeListener(handleChange)
  }, [])

  useEffect(() => {
    applyDocumentTheme(themeMode, resolvedTheme)
    void setAppTheme(themeMode === 'system' ? null : resolvedTheme)
  }, [resolvedTheme, themeMode])

  return { resolvedTheme, systemTheme }
}

export function normalizeThemeMode(value: unknown): ThemeMode {
  return value === 'light' || value === 'dark' || value === 'system' ? value : 'system'
}

function getSystemTheme(): ResolvedTheme {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return 'dark'
  return window.matchMedia(systemThemeQuery).matches ? 'dark' : 'light'
}

function applyDocumentTheme(themeMode: ThemeMode, resolvedTheme: ResolvedTheme) {
  if (typeof document === 'undefined') return

  const root = document.documentElement
  root.dataset.theme = themeMode
  root.dataset.themeMode = themeMode
  root.dataset.resolvedTheme = resolvedTheme
  root.classList.toggle('theme-light', themeMode === 'light')
  root.classList.toggle('theme-dark', themeMode === 'dark')
  root.classList.toggle('theme-system', themeMode === 'system')
  root.classList.toggle('theme-resolved-light', resolvedTheme === 'light')
  root.classList.toggle('theme-resolved-dark', resolvedTheme === 'dark')
  root.style.colorScheme = resolvedTheme

  document
    .querySelector('meta[name="color-scheme"]')
    ?.setAttribute('content', resolvedTheme)
}
