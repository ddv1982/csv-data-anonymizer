export function RiskBadge({ risk }: { risk: string }) {
  return <span className={`risk-badge risk-${risk}`}>{risk}</span>
}
