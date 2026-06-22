import { HelpCircle } from 'lucide-react'
import { type ReactNode, useEffect, useId, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { type GlossaryKey, glossaryTerms } from '../glossary'

type PopoverPosition = {
  top: number
  left: number
}

type HelpPopoverVariant = 'icon' | 'term'

type HelpPopoverProps = {
  title: string
  children: ReactNode
  triggerLabel: string
  triggerText?: ReactNode
  variant?: HelpPopoverVariant
}

export function GlossaryPopover({ term }: { term: GlossaryKey }) {
  const entry = glossaryTerms[term]
  return (
    <HelpPopover title={entry.title} triggerLabel={`Explain ${entry.title}`}>
      <p>{entry.body}</p>
    </HelpPopover>
  )
}

export function GlossaryTerm({
  term,
  children,
}: {
  term: GlossaryKey
  children: ReactNode
}) {
  const entry = glossaryTerms[term]

  return (
    <HelpPopover title={entry.title} triggerLabel={`Explain ${entry.title}`} triggerText={children} variant="term">
      <p>{entry.body}</p>
    </HelpPopover>
  )
}

export function HelpPopover({
  title,
  children,
  triggerLabel,
  triggerText,
  variant = 'icon',
}: HelpPopoverProps) {
  const triggerRef = useRef<HTMLButtonElement>(null)
  const panelRef = useRef<HTMLDivElement>(null)
  const [open, setOpen] = useState(false)
  const [position, setPosition] = useState<PopoverPosition | null>(null)
  const generatedId = useId()
  const panelId = `help-${generatedId}`
  const titleId = `${panelId}-title`
  const isTerm = variant === 'term'

  useEffect(() => {
    if (!open) return

    function updatePosition() {
      const trigger = triggerRef.current
      if (!trigger) return
      const rect = trigger.getBoundingClientRect()
      const viewportMargin = 8
      const preferredWidth = 288
      const panelWidth = Math.min(preferredWidth, window.innerWidth - viewportMargin * 2)
      const topBelow = rect.bottom + 8
      const estimatedHeight = 148
      const panelHeight = Math.min(
        panelRef.current?.offsetHeight ?? estimatedHeight,
        window.innerHeight - viewportMargin * 2,
      )
      const roomBelow = window.innerHeight - topBelow - viewportMargin
      const roomAbove = rect.top - viewportMargin
      const top =
        roomBelow >= panelHeight || roomBelow >= roomAbove
          ? Math.min(topBelow, window.innerHeight - panelHeight - viewportMargin)
          : Math.max(viewportMargin, rect.top - panelHeight - 8)
      const left = Math.min(
        Math.max(viewportMargin, rect.left + rect.width / 2 - panelWidth / 2),
        window.innerWidth - panelWidth - viewportMargin,
      )
      setPosition({ top, left })
    }

    updatePosition()
    const frame = window.requestAnimationFrame(updatePosition)
    window.addEventListener('resize', updatePosition)
    window.addEventListener('scroll', updatePosition, true)
    return () => {
      window.cancelAnimationFrame(frame)
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
    <span className={isTerm ? 'help-anchor glossary-term-anchor' : 'help-anchor glossary-anchor'}>
      <button
        ref={triggerRef}
        type="button"
        className={isTerm ? 'help-trigger glossary-term-trigger' : 'help-trigger glossary-trigger'}
        aria-label={isTerm ? undefined : triggerLabel}
        aria-expanded={open}
        aria-controls={open ? panelId : undefined}
        aria-describedby={open ? panelId : undefined}
        title={isTerm ? triggerLabel : undefined}
        onPointerDown={(event) => event.stopPropagation()}
        onClick={(event) => {
          event.stopPropagation()
          setOpen((current) => !current)
        }}
      >
        {isTerm ? null : <HelpCircle aria-hidden="true" />}
        {triggerText ? <span>{triggerText}</span> : null}
      </button>
      {open && position
        ? createPortal(
            <div
              ref={panelRef}
              id={panelId}
              role="tooltip"
              className="help-popover glossary-popover"
              style={{ top: position.top, left: position.left }}
            >
              <strong id={titleId} className="help-popover-title">
                {title}
              </strong>
              <div className="help-popover-content">{children}</div>
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
      <GlossaryTerm term={term}>{children}</GlossaryTerm>
    </span>
  )
}
