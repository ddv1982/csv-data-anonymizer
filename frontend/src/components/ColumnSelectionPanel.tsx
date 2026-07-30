import type { ReactNode } from 'react'

type SelectionAction = {
  label: string
  disabled: boolean
  onClick: () => void
}

/**
 * The chrome around a column table: bulk-select buttons, an optional notice above
 * the table and a summary below it.
 *
 * The table itself arrives as `children` rather than through forwarded props. This
 * used to re-declare every `ColumnTable` prop and pass it straight through, so each
 * call site drilled eleven props across two levels and every new table prop had to
 * be added in three files.
 */
export function ColumnSelectionPanel({
  actions,
  notice,
  footer,
  children,
}: {
  actions: SelectionAction[]
  notice?: ReactNode
  footer: ReactNode
  children: ReactNode
}) {
  return (
    <div className="columns-stack">
      <div className="bulk-actions">
        {actions.map((action) => (
          <button
            key={action.label}
            type="button"
            className="button button-outline button-sm"
            disabled={action.disabled}
            onClick={action.onClick}
          >
            {action.label}
          </button>
        ))}
      </div>

      {notice}

      {children}

      {footer}
    </div>
  )
}
