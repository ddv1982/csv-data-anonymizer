import { GlossaryPopover } from '../GlossaryPopover'
import { formatMetricValue, statusLabel, statusPillClass } from './helpers'
import type { PrivacyMetric } from './types'

export function PrivacyMetricGrid({
  metrics,
  variant,
}: {
  metrics: PrivacyMetric[]
  variant?: 'overview'
}) {
  return (
    <div className={variant === 'overview' ? 'privacy-metrics privacy-overview-metrics' : 'privacy-metrics'}>
      {metrics.map((metric) => (
        <div className="privacy-metric" key={metric.label}>
          <span className="privacy-metric-label muted-text text-sm">
            <span>{metric.label}</span>
            {metric.glossaryTerm ? <GlossaryPopover term={metric.glossaryTerm} /> : null}
          </span>
          <span className="privacy-metric-value-row">
            <strong>{formatMetricValue(metric.value)}</strong>
            {metric.status ? <span className={statusPillClass(metric.status)}>{statusLabel(metric.status)}</span> : null}
          </span>
          {metric.detail ? <p className="muted-text text-sm">{metric.detail}</p> : null}
        </div>
      ))}
    </div>
  )
}
