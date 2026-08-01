import { useCallback } from 'react'

const viewportNavigationKeys = new Set([
  'ArrowDown',
  'ArrowUp',
  'End',
  'Home',
  'PageDown',
  'PageUp',
  ' ',
])

/**
 * Keeps an action-triggered reflow from moving the viewport while allowing the
 * user to take control if they deliberately navigate during an async action.
 */
export function useStableViewportAction() {
  return useCallback(async <T,>(action: () => T | Promise<T>): Promise<T> => {
    const targetX = window.scrollX
    const targetY = window.scrollY
    let userMovedViewport = false
    let restoringViewport = false

    const markUserMovement = () => {
      if (!restoringViewport) userMovedViewport = true
    }
    const markKeyboardMovement = (event: KeyboardEvent) => {
      if (viewportNavigationKeys.has(event.key)) markUserMovement()
    }
    const listenerOptions: AddEventListenerOptions = { passive: true }

    window.addEventListener('wheel', markUserMovement, listenerOptions)
    window.addEventListener('touchmove', markUserMovement, listenerOptions)
    window.addEventListener('pointerdown', markUserMovement, listenerOptions)
    window.addEventListener('keydown', markKeyboardMovement)

    const restoreAfterRender = () => new Promise<void>((resolve) => {
      // Two frames let React commit and the browser finish focus/scroll anchoring
      // before we correct action-induced movement.
      window.requestAnimationFrame(() => window.requestAnimationFrame(() => {
        if (
          !userMovedViewport &&
          (Math.abs(window.scrollX - targetX) > 1 || Math.abs(window.scrollY - targetY) > 1)
        ) {
          restoringViewport = true
          window.scrollTo(targetX, targetY)
          window.requestAnimationFrame(() => {
            restoringViewport = false
            resolve()
          })
          return
        }
        resolve()
      }))
    })

    try {
      const result = action()
      await restoreAfterRender()
      const resolved = await result
      await restoreAfterRender()
      return resolved
    } finally {
      window.removeEventListener('wheel', markUserMovement)
      window.removeEventListener('touchmove', markUserMovement)
      window.removeEventListener('pointerdown', markUserMovement)
      window.removeEventListener('keydown', markKeyboardMovement)
    }
  }, [])
}
