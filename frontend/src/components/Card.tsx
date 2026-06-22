import type { ReactNode } from 'react'

export function Card({
  title,
  titleHelp,
  action,
  children,
  disabled = false,
  contentClassName = '',
}: {
  title?: ReactNode
  titleHelp?: ReactNode
  action?: ReactNode
  children: ReactNode
  disabled?: boolean
  contentClassName?: string
}) {
  return (
    <section className={disabled ? 'card section-disabled' : 'card'}>
      {title || titleHelp || action ? (
        <div className={action ? 'card-header card-header-row' : 'card-header'}>
          {title || titleHelp ? (
            <div className="card-title-row">
              {title ? <h2>{title}</h2> : null}
              {titleHelp}
            </div>
          ) : (
            <span />
          )}
          {action}
        </div>
      ) : null}
      <div className={contentClassName ? `card-content ${contentClassName}` : 'card-content'}>{children}</div>
    </section>
  )
}
