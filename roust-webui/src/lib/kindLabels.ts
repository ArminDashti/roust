import type { DestinationKind, TargetKind } from '@/api/client'

const KIND_LABELS: Record<string, string> = {
  cidr: 'CIDR',
  ip: 'IP',
  nic: 'NIC',
  mac: 'MAC',
  hostname: 'Hostname',
}

export function kindLabel(kind: TargetKind | DestinationKind | string): string {
  return KIND_LABELS[kind] ?? kind.toUpperCase()
}
