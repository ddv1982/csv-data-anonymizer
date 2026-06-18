import type { ReactNode } from 'react'

export function Card({
  title,
  action,
  children,
  disabled = false,
  contentClassName = '',
}: {
  title?: string
  action?: ReactNode
  children: ReactNode
  disabled?: boolean
  contentClassName?: string
}) {
  return (
    <section className={disabled ? 'card section-disabled' : 'card'}>
      {title || action ? (
        <div className={action ? 'card-header card-header-row' : 'card-header'}>
          {title ? <h2>{title}</h2> : <span />}
          {action}
        </div>
      ) : null}
      <div className={contentClassName ? `card-content ${contentClassName}` : 'card-content'}>{children}</div>
    </section>
  )
}
