import { ArrowRight } from 'lucide-react'
import type { PreviewData } from '../types'

export function PreviewTable({ preview, loading }: { preview: PreviewData | null; loading: boolean }) {
  if (loading) {
    return (
      <div className="preview-stack">
        {Array.from({ length: 2 }, (_, groupIndex) => (
          <div className="preview-group" key={groupIndex}>
            <span className="skeleton skeleton-preview-title" />
            {Array.from({ length: 3 }, (_, rowIndex) => (
              <div className="preview-row" key={rowIndex}>
                <span className="skeleton skeleton-preview-value" />
                <span className="skeleton skeleton-checkbox" />
                <span className="skeleton skeleton-preview-value" />
              </div>
            ))}
          </div>
        ))}
      </div>
    )
  }

  if (!preview || preview.previews.length === 0) {
    return <p className="empty-preview">No preview data available. Select columns and click "Show Preview".</p>
  }

  return (
    <div className="preview-stack">
      {preview.warnings.length > 0 ? (
        <div className="preview-group">
          <h3>Warnings</h3>
          <div className="preview-frame">
            {preview.warnings.map((warning) => (
              <p className="muted-text text-sm" key={`${warning.columnIndex}-${warning.message}`}>
                <strong>{warning.columnName}:</strong> {warning.message}
              </p>
            ))}
          </div>
        </div>
      ) : null}
      {preview.previews.map((columnPreview) => (
        <div className="preview-group" key={columnPreview.columnIndex}>
          <h3>
            {columnPreview.columnName} <span className="muted-text">(column {columnPreview.columnIndex})</span>
          </h3>
          <div className="preview-frame">
            {columnPreview.samples.length === 0 ? (
              <p className="muted-text italic text-sm">No data in sample rows for this column</p>
            ) : (
              columnPreview.samples.map((sample, index) => (
                <div className="preview-row" key={`${columnPreview.columnIndex}-${index}`}>
                  <code title={sample.original}>{sample.original}</code>
                  <ArrowRight aria-hidden="true" />
                  <code className="preview-output" title={sample.anonymized}>
                    {sample.anonymized}
                  </code>
                </div>
              ))
            )}
          </div>
        </div>
      ))}
    </div>
  )
}
