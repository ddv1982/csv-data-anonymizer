import type { ReactNode } from 'react'

export function ReportDisclosure({
  title,
  countLabel,
  children,
}: {
  title: string
  countLabel: string
  children: ReactNode
}) {
  return (
    <details className="report-disclosure">
      <summary>
        <span>{title}</span>
        <span className="status-pill">{countLabel}</span>
      </summary>
      <div className="report-disclosure-body">{children}</div>
    </details>
  )
}
