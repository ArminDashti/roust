<script setup lang="ts">
import { onMounted, ref } from 'vue'
import {
  api,
  type HostOverride,
  type HostOverrideItem,
} from '@/api/client'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
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
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { AlertCircle, Link2, Pencil, Plus, Trash2 } from 'lucide-vue-next'

const items = ref<HostOverrideItem[]>([])
const loading = ref(false)
const error = ref<string | null>(null)

const dialogOpen = ref(false)
const dialogMode = ref<'create' | 'edit'>('create')
const editingIndex = ref<number | null>(null)
const saving = ref(false)
const formError = ref<string | null>(null)

const hostname = ref('')
const ip = ref('')

const deleteOpen = ref(false)
const pendingDelete = ref<HostOverrideItem | null>(null)
const deleting = ref(false)

function overrideLabel(item: HostOverride) {
  return `${item.hostname} → ${item.ip}`
}

async function refresh() {
  loading.value = true
  error.value = null
  try {
    items.value = await api.listHostOverrides()
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}

function openCreate() {
  dialogMode.value = 'create'
  editingIndex.value = null
  hostname.value = ''
  ip.value = ''
  formError.value = null
  dialogOpen.value = true
}

function openEdit(item: HostOverrideItem) {
  dialogMode.value = 'edit'
  editingIndex.value = item.index
  hostname.value = item.hostname
  ip.value = item.ip
  formError.value = null
  dialogOpen.value = true
}

function buildPayload(): HostOverride {
  return {
    hostname: hostname.value.trim(),
    ip: ip.value.trim(),
  }
}

async function onSubmit() {
  saving.value = true
  formError.value = null
  try {
    const payload = buildPayload()
    if (!payload.hostname) throw new Error('Hostname is required')
    if (!payload.ip) throw new Error('IP is required')
    if (dialogMode.value === 'create') {
      await api.createHostOverride(payload)
    } else if (editingIndex.value != null) {
      await api.updateHostOverride(editingIndex.value, payload)
    }
    dialogOpen.value = false
    await refresh()
  } catch (e) {
    formError.value = e instanceof Error ? e.message : String(e)
  } finally {
    saving.value = false
  }
}

function onRemove(item: HostOverrideItem) {
  pendingDelete.value = item
  deleteOpen.value = true
}

async function confirmDelete() {
  const item = pendingDelete.value
  if (!item) return
  deleting.value = true
  error.value = null
  try {
    await api.deleteHostOverride(item.index)
    deleteOpen.value = false
    pendingDelete.value = null
    await refresh()
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    deleting.value = false
  }
}

onMounted(refresh)
</script>

<template>
  <div class="flex flex-col gap-6">
    <Alert v-if="error" variant="destructive">
      <AlertCircle class="size-4" />
      <AlertTitle>Host overrides error</AlertTitle>
      <AlertDescription>{{ error }}</AlertDescription>
    </Alert>

    <Card>
      <CardHeader class="flex flex-row items-start justify-between gap-4 space-y-0">
        <div class="space-y-1.5">
          <CardTitle class="flex items-center gap-2">
            <Link2 class="size-5" />
            Host overrides
          </CardTitle>
          <CardDescription>
            Force a hostname to always resolve to a fixed IPv4 (Windows hosts file).
            Stored in <code class="text-xs">host-overrides.json</code>.
            Writing hosts requires Administrator and must not be blocked by endpoint
            security (e.g. Kaspersky hosts-file protection).
          </CardDescription>
        </div>
        <Button size="sm" @click="openCreate">
          <Plus class="size-4" />
          Add override
        </Button>
      </CardHeader>
      <CardContent>
        <div v-if="loading && items.length === 0" class="text-sm text-muted-foreground">
          Loading host overrides…
        </div>
        <div
          v-else-if="items.length === 0"
          class="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground"
        >
          No host overrides yet. Map a hostname to a fixed IP.
        </div>
        <div v-else class="rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead class="w-14">#</TableHead>
                <TableHead>Hostname</TableHead>
                <TableHead>IP</TableHead>
                <TableHead class="w-28 text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="item in items" :key="item.index">
                <TableCell class="text-muted-foreground">{{ item.index }}</TableCell>
                <TableCell class="font-mono text-sm">{{ item.hostname }}</TableCell>
                <TableCell class="font-mono text-sm">{{ item.ip }}</TableCell>
                <TableCell class="text-right">
                  <div class="flex justify-end gap-1">
                    <Button
                      variant="ghost"
                      size="icon"
                      title="Edit"
                      @click="openEdit(item)"
                    >
                      <Pencil class="size-4" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      title="Delete"
                      @click="onRemove(item)"
                    >
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
            {{ dialogMode === 'create' ? 'Add host override' : 'Edit host override' }}
          </DialogTitle>
          <DialogDescription>
            This hostname will always resolve to the chosen IP (higher priority than DNS).
          </DialogDescription>
        </DialogHeader>
        <form class="grid gap-4 py-2" @submit.prevent="onSubmit">
          <div class="grid gap-2">
            <Label for="hostname">Hostname</Label>
            <Input
              id="hostname"
              v-model="hostname"
              placeholder="e.g. example.com"
              required
              autocomplete="off"
            />
          </div>
          <div class="grid gap-2">
            <Label for="ip">IP</Label>
            <Input
              id="ip"
              v-model="ip"
              placeholder="e.g. 203.0.113.10"
              required
              autocomplete="off"
            />
          </div>
          <p v-if="formError" class="text-sm text-destructive">{{ formError }}</p>
          <DialogFooter class="gap-2 sm:gap-0">
            <Button type="button" variant="outline" @click="dialogOpen = false">
              Cancel
            </Button>
            <Button type="submit" :disabled="saving">
              {{ saving ? 'Saving…' : dialogMode === 'create' ? 'Add' : 'Save' }}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>

    <Dialog v-model:open="deleteOpen">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Delete host override?</DialogTitle>
          <DialogDescription>
            {{ pendingDelete ? overrideLabel(pendingDelete) : '' }}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter class="gap-2 sm:gap-0">
          <Button
            type="button"
            variant="outline"
            :disabled="deleting"
            @click="deleteOpen = false"
          >
            Cancel
          </Button>
          <Button
            type="button"
            variant="destructive"
            :disabled="deleting"
            @click="confirmDelete"
          >
            {{ deleting ? 'Deleting…' : 'Delete' }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
