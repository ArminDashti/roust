<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import {
  api,
  type RouteItem,
  type RoutingRule,
} from '@/api/client'
import RouteFormDialog from '@/components/RouteFormDialog.vue'
import RoutesPanel from '@/components/RoutesPanel.vue'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { AlertCircle } from 'lucide-vue-next'
import { kindLabel } from '@/lib/kindLabels'

const routes = ref<RouteItem[]>([])
const loading = ref(false)
const error = ref<string | null>(null)

const dialogOpen = ref(false)
const dialogMode = ref<'create' | 'edit'>('create')
const editing = ref<RouteItem | null>(null)
const saving = ref(false)
const formError = ref<string | null>(null)

const deleteOpen = ref(false)
const pendingDelete = ref<RouteItem | null>(null)
const deleting = ref(false)

function ruleLabel(rule: RoutingRule) {
  return `${kindLabel(rule.target)}:${rule['target-value']} → ${kindLabel(rule.destination)}:${rule['destination-value']}`
}

const deleteTitle = computed(() =>
  pendingDelete.value?.source === 'system' ? 'Delete system route?' : 'Delete route?',
)

const deleteDescription = computed(() => {
  const route = pendingDelete.value
  if (!route) return ''
  if (route.source === 'system') {
    return `${ruleLabel(route)}\n\nIt will be adopted into App rules, then removed. The OS route may reappear as System afterward.`
  }
  return ruleLabel(route)
})

async function refresh() {
  loading.value = true
  error.value = null
  try {
    routes.value = await api.listRoutes()
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}

function openCreate() {
  dialogMode.value = 'create'
  editing.value = null
  formError.value = null
  dialogOpen.value = true
}

async function adoptRoute(route: RouteItem): Promise<RouteItem> {
  return api.createRoute({
    target: route.target,
    'target-value': route['target-value'],
    destination: route.destination,
    'destination-value': route['destination-value'],
  })
}

async function openEdit(route: RouteItem) {
  formError.value = null
  try {
    if (route.source === 'system') {
      saving.value = true
      const adopted = await adoptRoute(route)
      await refresh()
      editing.value = adopted
      dialogMode.value = 'edit'
      dialogOpen.value = true
    } else if (route.source === 'config' && route.index != null) {
      editing.value = route
      dialogMode.value = 'edit'
      dialogOpen.value = true
    }
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    saving.value = false
  }
}

async function onAdopt(route: RouteItem) {
  if (route.source !== 'system') return
  error.value = null
  try {
    await adoptRoute(route)
    await refresh()
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  }
}

async function onSubmit(payload: RoutingRule | RoutingRule[]) {
  saving.value = true
  formError.value = null
  try {
    if (dialogMode.value === 'create') {
      const rules = Array.isArray(payload) ? payload : [payload]
      if (rules.length > 1) {
        await api.createRoutesBatch(rules)
      } else if (rules.length === 1) {
        await api.createRoute(rules[0])
      }
    } else if (editing.value?.index != null) {
      const rule = Array.isArray(payload) ? payload[0] : payload
      if (!rule) throw new Error('Missing rule')
      await api.updateRoute(editing.value.index, rule)
    }
    dialogOpen.value = false
    await refresh()
  } catch (e) {
    formError.value = e instanceof Error ? e.message : String(e)
  } finally {
    saving.value = false
  }
}

function onRemove(route: RouteItem) {
  error.value = null
  pendingDelete.value = route
  deleteOpen.value = true
}

async function confirmDelete() {
  const route = pendingDelete.value
  if (!route) return
  deleting.value = true
  error.value = null
  try {
    if (route.source === 'system') {
      const adopted = await adoptRoute(route)
      if (adopted.index != null) {
        await api.deleteRoute(adopted.index)
      }
    } else if (route.source === 'config' && route.index != null) {
      await api.deleteRoute(route.index)
    }
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
      <AlertTitle>Routes error</AlertTitle>
      <AlertDescription>{{ error }}</AlertDescription>
    </Alert>

    <RoutesPanel
      :routes="routes"
      :loading="loading"
      @add="openCreate"
      @edit="openEdit"
      @remove="onRemove"
      @adopt="onAdopt"
    />

    <RouteFormDialog
      v-model:open="dialogOpen"
      :mode="dialogMode"
      :initial="editing"
      :saving="saving"
      :error="formError"
      @submit="onSubmit"
    />

    <Dialog v-model:open="deleteOpen">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{{ deleteTitle }}</DialogTitle>
          <DialogDescription class="whitespace-pre-line">
            {{ deleteDescription }}
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
