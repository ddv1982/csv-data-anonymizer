import { HelpCircle, X } from 'lucide-react'
import { useEffect, useId, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { type SectionHelpKey, sectionHelp } from '../sectionHelp'

export function SectionHelp({
  topic,
  label = 'How does this work?',
}: {
  topic: SectionHelpKey
  label?: string
}) {
  const entry = sectionHelp[topic]
  const [open, setOpen] = useState(false)
  const triggerRef = useRef<HTMLButtonElement>(null)
  const dialogRef = useRef<HTMLElement>(null)
  const closeButtonRef = useRef<HTMLButtonElement>(null)
  const generatedId = useId()
  const dialogId = `section-help-${generatedId}`
  const titleId = `${dialogId}-title`

  function close() {
    setOpen(false)
    window.requestAnimationFrame(() => triggerRef.current?.focus())
  }

  useEffect(() => {
    if (!open) return

    const previousOverflow = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    const frame = window.requestAnimationFrame(() => closeButtonRef.current?.focus())

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape') {
        event.preventDefault()
        close()
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
    }
  }, [open])

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        className="help-trigger section-help-trigger"
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls={open ? dialogId : undefined}
        onPointerDown={(event) => event.stopPropagation()}
        onClick={(event) => {
          event.stopPropagation()
          setOpen(true)
        }}
      >
        <HelpCircle aria-hidden="true" />
        <span>{label}</span>
      </button>
      {open
        ? createPortal(
            <div
              className="help-modal-backdrop"
              onPointerDown={(event) => {
                if (event.target === event.currentTarget) close()
              }}
            >
              <section
                id={dialogId}
                ref={dialogRef}
                className="help-modal"
                role="dialog"
                aria-modal="true"
                aria-labelledby={titleId}
                tabIndex={-1}
              >
                <header className="help-modal-header">
                  <div>
                    <span className="help-modal-eyebrow">Help article</span>
                    <h2 id={titleId}>{entry.title}</h2>
                  </div>
                  <button
                    ref={closeButtonRef}
                    type="button"
                    className="button button-ghost button-icon help-modal-close"
                    aria-label="Close help article"
                    onClick={close}
                  >
                    <X aria-hidden="true" />
                  </button>
                </header>
                <article className="help-modal-body">
                  <div className="help-article-summary">
                    {entry.summary.map((paragraph) => (
                      <p key={paragraph}>{paragraph}</p>
                    ))}
                  </div>
                  <dl className="help-detail-list help-article-list">
                    {entry.points.map((point) => (
                      <div key={point.label}>
                        <dt>{point.label}</dt>
                        <dd>{point.text}</dd>
                      </div>
                    ))}
                  </dl>
                </article>
              </section>
            </div>,
            document.body,
          )
        : null}
    </>
  )
}

function getFocusable(container: HTMLElement) {
  return Array.from(
    container.querySelectorAll<HTMLElement>(
      'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])',
    ),
  ).filter((element) => !element.hasAttribute('hidden') && element.offsetParent !== null)
}
