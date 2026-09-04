<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import {
  api,
  type AdapterItem,
  type AppBind,
  type AppBindItem,
  type AppBindStatus,
  type ProcessItem,
} from '@/api/client'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { AlertCircle, Link2, Pencil, Plus, Trash2 } from 'lucide-vue-next'

const binds = ref<AppBindItem[]>([])
const adapters = ref<AdapterItem[]>([])
const processes = ref<ProcessItem[]>([])
const loading = ref(false)
const error = ref<string | null>(null)

const dialogOpen = ref(false)
const dialogMode = ref<'create' | 'edit'>('create')
const editingIndex = ref<number | null>(null)
const saving = ref(false)
const formError = ref<string | null>(null)

const exePath = ref('')
const imageName = ref('')
const nic = ref('')
const processKey = ref('')

const nicOptions = computed(() =>
  adapters.value.map((a) => {
    const label = a.display_name || a.friendly_name || a.name
    const ip = a.ipv4_address ?? ''
    const status = a.status
    return {
      value: label,
      label: ip ? `${label} (${ip}, ${status})` : `${label} (${status})`,
    }
  }),
)

const processOptions = computed(() =>
  processes.value.map((p) => ({
    key: `${p.pid}:${p.image_name}`,
    label: p.exe_path
      ? `${p.image_name} — ${p.exe_path} (pid ${p.pid})`
      : `${p.image_name} (pid ${p.pid})`,
    process: p,
  })),
)

function statusVariant(status: AppBindStatus): 'default' | 'secondary' | 'destructive' | 'outline' {
  switch (status) {
    case 'healthy':
      return 'default'
    case 'nic-down':
      return 'destructive'
    default:
      return 'outline'
  }
}

function statusLabel(status: AppBindStatus) {
  switch (status) {
    case 'healthy':
      return 'Healthy'
    case 'nic-down':
      return 'NIC down'
    case 'unresolved':
      return 'Unresolved'
  }
}

function bindLabel(bind: AppBindItem) {
  const app = bind['exe-path'] || bind['image-name'] || '?'
  return `${app} → ${bind.nic}`
}

async function refresh() {
  loading.value = true
  error.value = null
  try {
    const [bindList, adapterList, processList] = await Promise.all([
      api.listAppBinds(),
      api.listAdapters(),
      api.listProcesses().catch(() => [] as ProcessItem[]),
    ])
    binds.value = bindList
    adapters.value = adapterList
    processes.value = processList
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}

function resetForm() {
  exePath.value = ''
  imageName.value = ''
  nic.value = nicOptions.value[0]?.value ?? ''
  processKey.value = ''
  formError.value = null
}

function openCreate() {
  dialogMode.value = 'create'
  editingIndex.value = null
  resetForm()
  dialogOpen.value = true
}

function openEdit(item: AppBindItem) {
  dialogMode.value = 'edit'
  editingIndex.value = item.index
  exePath.value = item['exe-path'] ?? ''
  imageName.value = item['image-name'] ?? ''
  nic.value = item.nic
  processKey.value = ''
  formError.value = null
  dialogOpen.value = true
}

function onProcessPicked(value: unknown) {
  if (typeof value !== 'string' || !value) return
  processKey.value = value
  const opt = processOptions.value.find((o) => o.key === value)
  if (!opt) return
  imageName.value = opt.process.image_name
  if (opt.process.exe_path) {
    exePath.value = opt.process.exe_path
  }
}

function buildPayload(): AppBind {
  const payload: AppBind = { nic: nic.value.trim() }
  const path = exePath.value.trim()
  const image = imageName.value.trim()
  if (path) payload['exe-path'] = path
  if (image) payload['image-name'] = image
  return payload
}

async function onSubmit() {
  saving.value = true
  formError.value = null
  try {
    const payload = buildPayload()
    if (!payload.nic) throw new Error('Select a NIC')
    if (!payload['exe-path'] && !payload['image-name']) {
      throw new Error('Provide an exe path and/or image name')
    }
    if (dialogMode.value === 'create') {
      await api.createAppBind(payload)
    } else if (editingIndex.value != null) {
      await api.updateAppBind(editingIndex.value, payload)
    }
    dialogOpen.value = false
    await refresh()
  } catch (e) {
    formError.value = e instanceof Error ? e.message : String(e)
  } finally {
    saving.value = false
  }
}

async function onRemove(item: AppBindItem) {
  error.value = null
  const ok = window.confirm(`Delete app bind?\n\n${bindLabel(item)}`)
  if (!ok) return
  try {
    await api.deleteAppBind(item.index)
    await refresh()
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  }
}

onMounted(refresh)
</script>

<template>
  <div class="flex flex-col gap-6">
    <Alert v-if="error" variant="destructive">
      <AlertCircle class="size-4" />
      <AlertTitle>App binds error</AlertTitle>
      <AlertDescription>{{ error }}</AlertDescription>
    </Alert>

    <Card>
      <CardHeader class="flex flex-row items-start justify-between gap-4 space-y-0">
        <div class="space-y-1.5">
          <CardTitle class="flex items-center gap-2">
            <Link2 class="size-5" />
            App binds
          </CardTitle>
          <CardDescription>
            Force a process through a chosen NIC via Windows Filtering Platform.
            Fail-closed: if the NIC is down, that app’s outbound traffic is blocked.
            Stored in <code class="text-xs">app-binds.json</code>.
          </CardDescription>
        </div>
        <Button size="sm" @click="openCreate">
          <Plus class="size-4" />
          Add bind
        </Button>
      </CardHeader>
      <CardContent>
        <div v-if="loading && binds.length === 0" class="text-sm text-muted-foreground">
          Loading app binds…
        </div>
        <div
          v-else-if="binds.length === 0"
          class="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground"
        >
          No app binds yet. Pick a process and NIC to force its egress.
        </div>
        <div v-else class="rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead class="w-14">#</TableHead>
                <TableHead>App</TableHead>
                <TableHead>NIC</TableHead>
                <TableHead class="w-32">Status</TableHead>
                <TableHead class="w-36 text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="item in binds" :key="item.index">
                <TableCell class="font-mono text-muted-foreground">
                  {{ item.index }}
                </TableCell>
                <TableCell>
                  <div class="space-y-1">
                    <div class="font-mono text-sm">
                      {{ item['image-name'] || item['exe-path'] || '—' }}
                    </div>
                    <div
                      v-if="item['exe-path'] && item['image-name']"
                      class="truncate text-xs text-muted-foreground"
                      :title="item['exe-path']"
                    >
                      {{ item['exe-path'] }}
                    </div>
                  </div>
                </TableCell>
                <TableCell class="font-mono text-sm">{{ item.nic }}</TableCell>
                <TableCell>
                  <Badge :variant="statusVariant(item.status)">
                    {{ statusLabel(item.status) }}
                  </Badge>
                </TableCell>
                <TableCell class="text-right">
                  <div class="flex justify-end gap-1">
                    <Button variant="ghost" size="sm" @click="openEdit(item)">
                      <Pencil class="size-4" />
                    </Button>
                    <Button variant="ghost" size="sm" @click="onRemove(item)">
                      <Trash2 class="size-4" />
                    </Button>
                  </div>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>
      </CardContent>
    </Card>

    <Dialog v-model:open="dialogOpen">
      <DialogContent class="max-w-lg">
        <DialogHeader>
          <DialogTitle>
            {{ dialogMode === 'create' ? 'Add app bind' : 'Edit app bind' }}
          </DialogTitle>
          <DialogDescription>
            Prefer a full exe path. Image name alone resolves from a running process at enforce time.
          </DialogDescription>
        </DialogHeader>

        <div class="grid gap-4 py-2">
          <Alert v-if="formError" variant="destructive">
            <AlertCircle class="size-4" />
            <AlertTitle>Save failed</AlertTitle>
            <AlertDescription>{{ formError }}</AlertDescription>
          </Alert>

          <div class="grid gap-2">
            <Label>Running process (optional)</Label>
            <Select :model-value="processKey" @update:model-value="onProcessPicked">
              <SelectTrigger class="w-full">
                <SelectValue placeholder="Pick a process to fill path" />
              </SelectTrigger>
              <SelectContent class="max-h-72">
                <SelectItem
                  v-for="opt in processOptions"
                  :key="opt.key"
                  :value="opt.key"
                >
                  {{ opt.label }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div class="grid gap-2">
            <Label for="bind-exe">Exe path</Label>
            <Input
              id="bind-exe"
              v-model="exePath"
              placeholder="C:\Program Files\App\App.exe"
              autocomplete="off"
              :disabled="saving"
            />
          </div>

          <div class="grid gap-2">
            <Label for="bind-image">Image name</Label>
            <Input
              id="bind-image"
              v-model="imageName"
              placeholder="App.exe"
              autocomplete="off"
              :disabled="saving"
            />
          </div>

          <div class="grid gap-2">
            <Label for="bind-nic">NIC</Label>
            <Select v-model="nic" :disabled="saving">
              <SelectTrigger id="bind-nic" class="w-full">
                <SelectValue placeholder="Select NIC" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="opt in nicOptions"
                  :key="opt.value"
                  :value="opt.value"
                >
                  {{ opt.label }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" :disabled="saving" @click="dialogOpen = false">
            Cancel
          </Button>
          <Button :disabled="saving" @click="onSubmit">
            {{ saving ? 'Saving…' : 'Save' }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
