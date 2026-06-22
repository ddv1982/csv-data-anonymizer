import { type SectionHelpKey, sectionHelp } from '../sectionHelp'
import { HelpPopover } from './GlossaryPopover'

export function SectionHelp({ topic }: { topic: SectionHelpKey }) {
  const entry = sectionHelp[topic]

  return (
    <HelpPopover
      title={entry.title}
      triggerLabel={`How does this work? ${entry.title}`}
      triggerText="How does this work?"
      variant="section"
    >
      {entry.summary.map((paragraph) => (
        <p key={paragraph}>{paragraph}</p>
      ))}
      <dl className="help-detail-list">
        {entry.points.map((point) => (
          <div key={point.label}>
            <dt>{point.label}</dt>
            <dd>{point.text}</dd>
          </div>
        ))}
      </dl>
    </HelpPopover>
  )
}
