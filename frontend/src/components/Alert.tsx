import type { ReactNode } from 'react'

export function Alert({
  icon,
  variant,
  children,
}: {
  icon: ReactNode
  variant?: 'destructive' | 'success'
  children: ReactNode
}) {
  return (
    <div className={variant ? `alert alert-${variant}` : 'alert'} role="alert">
      <span className="alert-icon">{icon}</span>
      <div className="alert-description">{children}</div>
    </div>
  )
}
