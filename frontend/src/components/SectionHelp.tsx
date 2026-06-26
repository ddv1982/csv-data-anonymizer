import { HelpCircle } from 'lucide-react'
import { Fragment, type ReactNode, useId, useState } from 'react'
import { type HelpText, type HelpTextSegment, type SectionHelpKey, sectionHelp } from '../sectionHelp'
import { GlossaryTerm } from './GlossaryPopover'
import { ModalDialog } from './ModalDialog'

export function SectionHelp({
  topic,
  label = 'How does this work?',
}: {
  topic: SectionHelpKey
  label?: string
}) {
  const entry = sectionHelp[topic]
  const [open, setOpen] = useState(false)
  const generatedId = useId()
  const dialogId = `section-help-${generatedId}`
  const titleId = `${dialogId}-title`

  return (
    <>
      <button
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
      <ModalDialog
        open={open}
        title={entry.title}
        eyebrow="Help article"
        dialogId={dialogId}
        titleId={titleId}
        closeLabel="Close help article"
        onClose={() => setOpen(false)}
      >
        <div className="help-article-summary">
          {entry.summary.map((paragraph, index) => (
            <p key={`summary-${index}`}>{renderHelpText(paragraph)}</p>
          ))}
        </div>
        <dl className="help-detail-list help-article-list">
          {entry.points.map((point) => (
            <div key={point.label}>
              <dt>{point.label}</dt>
              <dd>{renderHelpText(point.text)}</dd>
            </div>
          ))}
        </dl>
      </ModalDialog>
    </>
  )
}

function renderHelpText(text: HelpText): ReactNode {
  if (typeof text === 'string') return text
  return text.map((segment, index) => renderHelpTextSegment(segment, index))
}

function renderHelpTextSegment(segment: HelpTextSegment, index: number): ReactNode {
  if (typeof segment === 'string') {
    return <Fragment key={`text-${index}`}>{segment}</Fragment>
  }

  return (
    <GlossaryTerm key={`${segment.term}-${index}`} term={segment.term}>
      {segment.text}
    </GlossaryTerm>
  )
}
