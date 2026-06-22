import { HelpCircle } from 'lucide-react'
import { type ReactNode, useEffect, useId, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { type GlossaryKey, glossaryTerms } from '../glossary'

type PopoverPosition = {
  top: number
  left: number
}

export function GlossaryPopover({ term }: { term: GlossaryKey }) {
  const entry = glossaryTerms[term]
  const triggerRef = useRef<HTMLButtonElement>(null)
  const panelRef = useRef<HTMLDivElement>(null)
  const [open, setOpen] = useState(false)
  const [position, setPosition] = useState<PopoverPosition | null>(null)
  const generatedId = useId()
  const panelId = `glossary-${generatedId}`

  useEffect(() => {
    if (!open) return

    function updatePosition() {
      const trigger = triggerRef.current
      if (!trigger) return
      const rect = trigger.getBoundingClientRect()
      const panelWidth = Math.min(288, window.innerWidth - 16)
      const topBelow = rect.bottom + 8
      const estimatedHeight = 148
      const hasRoomBelow = topBelow + estimatedHeight <= window.innerHeight - 8
      const top = hasRoomBelow ? topBelow : Math.max(8, rect.top - estimatedHeight - 8)
      const left = Math.min(Math.max(8, rect.left + rect.width / 2 - panelWidth / 2), window.innerWidth - panelWidth - 8)
      setPosition({ top, left })
    }

    updatePosition()
    window.addEventListener('resize', updatePosition)
    window.addEventListener('scroll', updatePosition, true)
    return () => {
      window.removeEventListener('resize', updatePosition)
      window.removeEventListener('scroll', updatePosition, true)
    }
  }, [open])

  useEffect(() => {
    if (!open) return

    function closeIfOutside(event: PointerEvent) {
      const target = event.target
      if (!(target instanceof Node)) return
      if (triggerRef.current?.contains(target) || panelRef.current?.contains(target)) return
      setOpen(false)
    }

    function closeOnFocusMove(event: FocusEvent) {
      const target = event.target
      if (!(target instanceof Node)) return
      if (triggerRef.current?.contains(target) || panelRef.current?.contains(target)) return
      setOpen(false)
    }

    function closeOnEscape(event: KeyboardEvent) {
      if (event.key !== 'Escape') return
      setOpen(false)
      triggerRef.current?.focus()
    }

    document.addEventListener('pointerdown', closeIfOutside, true)
    document.addEventListener('focusin', closeOnFocusMove)
    document.addEventListener('keydown', closeOnEscape)
    return () => {
      document.removeEventListener('pointerdown', closeIfOutside, true)
      document.removeEventListener('focusin', closeOnFocusMove)
      document.removeEventListener('keydown', closeOnEscape)
    }
  }, [open])

  return (
    <span className="glossary-anchor">
      <button
        ref={triggerRef}
        type="button"
        className="glossary-trigger"
        aria-label={`Explain ${entry.title}`}
        aria-expanded={open}
        aria-controls={open ? panelId : undefined}
        aria-describedby={open ? panelId : undefined}
        onPointerDown={(event) => event.stopPropagation()}
        onClick={(event) => {
          event.stopPropagation()
          setOpen((current) => !current)
        }}
      >
        <HelpCircle aria-hidden="true" />
      </button>
      {open && position
        ? createPortal(
            <div
              ref={panelRef}
              id={panelId}
              role="tooltip"
              className="glossary-popover"
              style={{ top: position.top, left: position.left }}
            >
              <strong>{entry.title}</strong>
              <p>{entry.body}</p>
            </div>,
            document.body,
          )
        : null}
    </span>
  )
}

export function GlossaryLabel({
  term,
  children,
  className = '',
}: {
  term: GlossaryKey
  children: ReactNode
  className?: string
}) {
  return (
    <span className={className ? `glossary-label ${className}` : 'glossary-label'}>
      <span>{children}</span>
      <GlossaryPopover term={term} />
    </span>
  )
}
