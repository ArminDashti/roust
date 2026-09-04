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
  ipv4_address: string | null
  status: string
}

export interface PingReply {
  seq: number
  success: boolean
  rtt_ms?: number
  error?: string
}

export interface PingResult {
  host: string
  dest_ip: string
  nic: string
  source_ip: string
  sent: number
  received: number
  loss_pct: number
  route_restored: boolean
  replies: PingReply[]
}

export interface PingRequest {
  host: string
  nic: string
  count?: number
}

export interface ServiceActionResponse {
  ok: boolean
  installed: boolean
  state: string
}

export interface ProcessItem {
  pid: number
  image_name: string
  exe_path?: string
}

export type AppBindStatus = 'healthy' | 'nic-down' | 'unresolved'

export interface AppBind {
  'exe-path'?: string
  'image-name'?: string
  nic: string
}

export interface AppBindItem extends AppBind {
  index: number
  status: AppBindStatus
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
  ping: (body: PingRequest) =>
    request<PingResult>('/ping', {
      method: 'POST',
      body: JSON.stringify(body),
    }),
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
  listProcesses: () => request<ProcessItem[]>('/processes'),
  listAppBinds: () => request<AppBindItem[]>('/app-binds'),
  createAppBind: (bind: AppBind) =>
    request<AppBindItem>('/app-binds', {
      method: 'POST',
      body: JSON.stringify(bind),
    }),
  updateAppBind: (index: number, bind: AppBind) =>
    request<AppBindItem>(`/app-binds/${index}`, {
      method: 'PUT',
      body: JSON.stringify(bind),
    }),
  deleteAppBind: (index: number) =>
    request<void>(`/app-binds/${index}`, { method: 'DELETE' }),
  installService: () =>
    request<ServiceActionResponse>('/service/install', { method: 'POST' }),
  startService: () =>
    request<ServiceActionResponse>('/service/start', { method: 'POST' }),
  stopService: () =>
    request<ServiceActionResponse>('/service/stop', { method: 'POST' }),
  restartService: () =>
    request<ServiceActionResponse>('/service/restart', { method: 'POST' }),
}
