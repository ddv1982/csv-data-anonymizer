import type { PiiRisk } from '../types'

export function RiskBadge({ risk }: { risk: PiiRisk }) {
  return <span className={`risk-badge risk-${risk}`}>{risk}</span>
}
