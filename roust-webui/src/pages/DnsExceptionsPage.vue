<script setup lang="ts">
import { onMounted, ref } from 'vue'
import {
  api,
  type DnsException,
  type DnsExceptionItem,
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
import { AlertCircle, Globe, Pencil, Plus, Trash2 } from 'lucide-vue-next'

const items = ref<DnsExceptionItem[]>([])
const loading = ref(false)
const error = ref<string | null>(null)

const dialogOpen = ref(false)
const dialogMode = ref<'create' | 'edit'>('create')
const editingIndex = ref<number | null>(null)
const saving = ref(false)
const formError = ref<string | null>(null)

const namespace = ref('')
const dnsServer = ref('')

const deleteOpen = ref(false)
const pendingDelete = ref<DnsExceptionItem | null>(null)
const deleting = ref(false)

function exceptionLabel(item: DnsException) {
  return `${item.namespace} → ${item['dns-server']}`
}

async function refresh() {
  loading.value = true
  error.value = null
  try {
    items.value = await api.listDnsExceptions()
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}

function openCreate() {
  dialogMode.value = 'create'
  editingIndex.value = null
  namespace.value = ''
  dnsServer.value = ''
  formError.value = null
  dialogOpen.value = true
}

function openEdit(item: DnsExceptionItem) {
  dialogMode.value = 'edit'
  editingIndex.value = item.index
  namespace.value = item.namespace
  dnsServer.value = item['dns-server']
  formError.value = null
  dialogOpen.value = true
}

function buildPayload(): DnsException {
  return {
    namespace: namespace.value.trim(),
    'dns-server': dnsServer.value.trim(),
  }
}

async function onSubmit() {
  saving.value = true
  formError.value = null
  try {
    const payload = buildPayload()
    if (!payload.namespace) throw new Error('Namespace is required')
    if (!payload['dns-server']) throw new Error('DNS server is required')
    if (dialogMode.value === 'create') {
      await api.createDnsException(payload)
    } else if (editingIndex.value != null) {
      await api.updateDnsException(editingIndex.value, payload)
    }
    dialogOpen.value = false
    await refresh()
  } catch (e) {
    formError.value = e instanceof Error ? e.message : String(e)
  } finally {
    saving.value = false
  }
}

function onRemove(item: DnsExceptionItem) {
  pendingDelete.value = item
  deleteOpen.value = true
}

async function confirmDelete() {
  const item = pendingDelete.value
  if (!item) return
  deleting.value = true
  error.value = null
  try {
    await api.deleteDnsException(item.index)
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
      <AlertTitle>DNS exceptions error</AlertTitle>
      <AlertDescription>{{ error }}</AlertDescription>
    </Alert>

    <Card>
      <CardHeader class="flex flex-row items-start justify-between gap-4 space-y-0">
        <div class="space-y-1.5">
          <CardTitle class="flex items-center gap-2">
            <Globe class="size-5" />
            DNS exceptions
          </CardTitle>
          <CardDescription>
            Resolve selected hostnames via a specific DNS server (Windows NRPT).
            Stored in <code class="text-xs">dns-exceptions.json</code>.
          </CardDescription>
        </div>
        <Button size="sm" @click="openCreate">
          <Plus class="size-4" />
          Add exception
        </Button>
      </CardHeader>
      <CardContent>
        <div v-if="loading && items.length === 0" class="text-sm text-muted-foreground">
          Loading DNS exceptions…
        </div>
        <div
          v-else-if="items.length === 0"
          class="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground"
        >
          No DNS exceptions yet. Map a hostname to a DNS server IP.
        </div>
        <div v-else class="rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead class="w-14">#</TableHead>
                <TableHead>Namespace</TableHead>
                <TableHead>DNS server</TableHead>
                <TableHead class="w-28 text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="item in items" :key="item.index">
                <TableCell class="text-muted-foreground">{{ item.index }}</TableCell>
                <TableCell class="font-mono text-sm">{{ item.namespace }}</TableCell>
                <TableCell class="font-mono text-sm">{{ item['dns-server'] }}</TableCell>
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
            {{ dialogMode === 'create' ? 'Add DNS exception' : 'Edit DNS exception' }}
          </DialogTitle>
          <DialogDescription>
            Names under this namespace resolve through the chosen DNS server.
          </DialogDescription>
        </DialogHeader>
        <form class="grid gap-4 py-2" @submit.prevent="onSubmit">
          <div class="grid gap-2">
            <Label for="namespace">Namespace</Label>
            <Input
              id="namespace"
              v-model="namespace"
              placeholder="e.g. example.com"
              required
              autocomplete="off"
            />
          </div>
          <div class="grid gap-2">
            <Label for="dns-server">DNS server</Label>
            <Input
              id="dns-server"
              v-model="dnsServer"
              placeholder="e.g. 8.8.8.8"
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
          <DialogTitle>Delete DNS exception?</DialogTitle>
          <DialogDescription>
            {{ pendingDelete ? exceptionLabel(pendingDelete) : '' }}
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
