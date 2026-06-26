import { X } from 'lucide-react'
import { type ReactNode, useEffect, useId, useRef } from 'react'
import { createPortal } from 'react-dom'

export function ModalDialog({
  open,
  title,
  eyebrow,
  children,
  onClose,
  closeLabel = 'Close dialog',
  dialogId,
  titleId,
  className,
  bodyClassName,
}: {
  open: boolean
  title: string
  eyebrow?: string
  children: ReactNode
  onClose: () => void
  closeLabel?: string
  dialogId?: string
  titleId?: string
  className?: string
  bodyClassName?: string
}) {
  const generatedId = useId()
  const resolvedDialogId = dialogId ?? `modal-${generatedId}`
  const resolvedTitleId = titleId ?? `${resolvedDialogId}-title`
  const dialogRef = useRef<HTMLElement>(null)
  const closeButtonRef = useRef<HTMLButtonElement>(null)
  const restoreFocusRef = useRef<HTMLElement | null>(null)
  const onCloseRef = useRef(onClose)

  useEffect(() => {
    onCloseRef.current = onClose
  }, [onClose])

  useEffect(() => {
    if (!open) return

    restoreFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null
    const previousOverflow = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    const frame = window.requestAnimationFrame(() => closeButtonRef.current?.focus())

    function handleKeyDown(event: KeyboardEvent) {
      if (!isTopmostDialog(dialogRef.current)) return

      if (event.key === 'Escape') {
        event.preventDefault()
        onCloseRef.current()
        return
      }

      if (event.key !== 'Tab') return
      const dialog = dialogRef.current
      if (!dialog) return
      const focusable = getFocusable(dialog)
      if (focusable.length === 0) {
        event.preventDefault()
        dialog.focus()
        return
      }

      const first = focusable[0]
      const last = focusable[focusable.length - 1]
      const active = document.activeElement
      if (event.shiftKey && active === first) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && active === last) {
        event.preventDefault()
        first.focus()
      }
    }

    document.addEventListener('keydown', handleKeyDown)
    return () => {
      window.cancelAnimationFrame(frame)
      document.body.style.overflow = previousOverflow
      document.removeEventListener('keydown', handleKeyDown)
      window.requestAnimationFrame(() => restoreFocusRef.current?.focus())
    }
  }, [open])

  if (!open) return null

  return createPortal(
    <div
      className="help-modal-backdrop"
      onPointerDown={(event) => {
        if (event.target === event.currentTarget) onClose()
      }}
    >
      <section
        id={resolvedDialogId}
        ref={dialogRef}
        className={className ? `help-modal ${className}` : 'help-modal'}
        role="dialog"
        aria-modal="true"
        aria-labelledby={resolvedTitleId}
        tabIndex={-1}
      >
        <header className="help-modal-header">
          <div>
            {eyebrow ? <span className="help-modal-eyebrow">{eyebrow}</span> : null}
            <h2 id={resolvedTitleId}>{title}</h2>
          </div>
          <button
            ref={closeButtonRef}
            type="button"
            className="button button-ghost button-icon help-modal-close"
            aria-label={closeLabel}
            onClick={onClose}
          >
            <X aria-hidden="true" />
          </button>
        </header>
        <article className={bodyClassName ? `help-modal-body ${bodyClassName}` : 'help-modal-body'}>
          {children}
        </article>
      </section>
    </div>,
    document.body,
  )
}

function isTopmostDialog(dialog: HTMLElement | null) {
  if (!dialog) return false
  const dialogs = Array.from(document.querySelectorAll<HTMLElement>('[role="dialog"][aria-modal="true"]'))
  return dialogs[dialogs.length - 1] === dialog
}

function getFocusable(container: HTMLElement) {
  return Array.from(
    container.querySelectorAll<HTMLElement>(
      'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])',
    ),
  ).filter((element) => !element.hasAttribute('hidden') && element.offsetParent !== null)
}
