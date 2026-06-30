import type { ColumnReleaseReport } from '../../types'
import { formatToken } from '../../utils/format'
import { RiskBadge } from '../RiskBadge'
import { statusLabel, statusPillClass } from './helpers'

export function PrivacyReportColumnDecisions({ columns }: { columns: ColumnReleaseReport[] }) {
  const visibleColumns = columns.slice(0, 12)

  return (
    <div className="report-subsection">
      <div className="report-subsection-header">
        <h4>Column Decisions</h4>
        <span className="status-pill">
          Showing {visibleColumns.length.toLocaleString()} of {columns.length.toLocaleString()}
        </span>
      </div>
      <div className="table-frame release-column-frame">
        <table className="release-column-table" aria-label="Privacy report column decisions">
          <colgroup>
            <col className="release-column-name-col" />
            <col className="release-column-risk-col" />
            <col className="release-column-strategy-col" />
            <col className="release-column-status-col" />
            <col className="release-column-action-col" />
          </colgroup>
          <thead>
            <tr>
              <th>Column</th>
              <th className="release-risk-heading">Risk</th>
              <th>Strategy</th>
              <th>Status</th>
              <th>Action</th>
            </tr>
          </thead>
          <tbody>
            {visibleColumns.map((column) => (
              <tr key={`${column.columnIndex}-${column.columnName}`}>
                <td className="release-column-name-cell">
                  <strong>{column.columnName}</strong>
                  <span className="muted-text text-sm">
                    #{column.columnIndex} / {formatToken(column.detectedType)}
                  </span>
                </td>
                <td className="release-risk-cell">
                  <RiskBadge risk={column.piiRisk} />
                </td>
                <td>{formatToken(column.strategy)}</td>
                <td>
                  <span className={statusPillClass(column.status)}>{statusLabel(column.status)}</span>
                </td>
                <td className="release-action-cell">
                  <details className="column-action-details">
                    <summary>{column.action}</summary>
                    <p className="muted-text text-sm">{column.detail}</p>
                  </details>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}
