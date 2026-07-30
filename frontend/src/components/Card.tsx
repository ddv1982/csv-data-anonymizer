import type { ReactNode } from 'react'

export function Card({
  title,
  action,
  children,
  disabled = false,
  contentClassName = '',
}: {
  title?: ReactNode
  action?: ReactNode
  children: ReactNode
  disabled?: boolean
  contentClassName?: string
}) {
  return (
    <section className={disabled ? 'card section-disabled' : 'card'}>
      {title || action ? (
        <div className={action ? 'card-header card-header-row' : 'card-header'}>
          {title ? (
            <div className="card-title-row">
              <h2>{title}</h2>
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
