export type TargetKind = 'nic' | 'ip' | 'cidr' | 'mac'
export type DestinationKind = 'nic' | 'ip' | 'mac'

export interface StatusResponse {
  installed: boolean
  state: string
  config_path: string
  rule_count: number
  config_rule_count: number
  system_rule_count: number
  version: string
}

export type RouteSource = 'config' | 'system'

export interface RoutingRule {
  target: TargetKind
  'target-value': string
  destination: DestinationKind
  'destination-value': string
}

export interface RouteItem extends RoutingRule {
  index?: number
  source: RouteSource
}

export interface AdapterItem {
  name: string
  display_name: string
  friendly_name: string | null
  mac_address: string
  if_index: number
}

export interface ServiceActionResponse {
  ok: boolean
  installed: boolean
  state: string
}

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...(options.headers as Record<string, string> | undefined),
  }

  const base = import.meta.env.BASE_URL || '/'
  const apiRoot = `${base.endsWith('/') ? base : `${base}/`}api`
  const res = await fetch(`${apiRoot}${path}`, { ...options, headers })

  if (!res.ok) {
    const body = (await res.json().catch(() => ({ error: res.statusText }))) as {
      error?: string
    }
    throw new Error(body.error || `Request failed (${res.status})`)
  }

  if (res.status === 204) {
    return undefined as T
  }

  return res.json() as Promise<T>
}

export const api = {
  getStatus: () => request<StatusResponse>('/status'),
  listRoutes: () => request<RouteItem[]>('/routes'),
  listAdapters: () => request<AdapterItem[]>('/adapters'),
  createRoute: (rule: RoutingRule) =>
    request<RouteItem>('/routes', {
      method: 'POST',
      body: JSON.stringify(rule),
    }),
  createRoutesBatch: (rules: RoutingRule[]) =>
    request<RouteItem[]>('/routes/batch', {
      method: 'POST',
      body: JSON.stringify({ rules }),
    }),
  updateRoute: (index: number, rule: RoutingRule) =>
    request<RouteItem>(`/routes/${index}`, {
      method: 'PUT',
      body: JSON.stringify(rule),
    }),
  deleteRoute: (index: number) =>
    request<void>(`/routes/${index}`, { method: 'DELETE' }),
  installService: () =>
    request<ServiceActionResponse>('/service/install', { method: 'POST' }),
  startService: () =>
    request<ServiceActionResponse>('/service/start', { method: 'POST' }),
  stopService: () =>
    request<ServiceActionResponse>('/service/stop', { method: 'POST' }),
  restartService: () =>
    request<ServiceActionResponse>('/service/restart', { method: 'POST' }),
}
