<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { api, type StatusResponse } from '@/api/client'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { AlertCircle, Play, Power, RotateCcw, Settings, Square } from 'lucide-vue-next'

const status = ref<StatusResponse | null>(null)
const loading = ref(false)
const acting = ref(false)
const error = ref<string | null>(null)

const installed = computed(() => status.value?.installed ?? false)
const state = computed(() => status.value?.state ?? '')
const isRunning = computed(() => state.value === 'Running')
const isStopped = computed(() => state.value === 'Stopped')

async function refresh() {
  loading.value = true
  error.value = null
  try {
    status.value = await api.getStatus()
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}

async function runAction(action: () => Promise<unknown>) {
  acting.value = true
  error.value = null
  try {
    await action()
    await refresh()
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    acting.value = false
  }
}

onMounted(refresh)
</script>

<template>
  <div class="flex flex-col gap-6">
    <Alert v-if="error" variant="destructive">
      <AlertCircle class="size-4" />
      <AlertTitle>Service action failed</AlertTitle>
      <AlertDescription>{{ error }}</AlertDescription>
    </Alert>

    <Card>
      <CardHeader>
        <CardTitle class="flex items-center gap-2">
          <Settings class="size-5" />
          Settings
        </CardTitle>
        <CardDescription>
          Install and control the Roust Windows service. Config path is read-only.
        </CardDescription>
      </CardHeader>
      <CardContent class="space-y-6">
        <div class="space-y-1">
          <p class="text-xs font-medium text-muted-foreground">Config path</p>
          <p
            class="break-all font-mono text-sm"
            :title="status?.config_path"
          >
            {{ loading && !status ? 'Loading…' : status?.config_path ?? '—' }}
          </p>
          <p class="text-xs text-muted-foreground">
            State:
            <span class="font-medium text-foreground">{{ state || '—' }}</span>
            ·
            {{ installed ? 'Installed' : 'Not installed' }}
          </p>
        </div>

        <div class="flex flex-wrap gap-2">
          <Button
            size="sm"
            variant="outline"
            :disabled="acting || installed"
            @click="runAction(() => api.installService())"
          >
            <Power class="size-4" />
            Install
          </Button>
          <Button
            size="sm"
            variant="outline"
            :disabled="acting || !installed || isRunning"
            @click="runAction(() => api.startService())"
          >
            <Play class="size-4" />
            Start
          </Button>
          <Button
            size="sm"
            variant="outline"
            :disabled="acting || !installed || isStopped"
            @click="runAction(() => api.stopService())"
          >
            <Square class="size-4" />
            Stop
          </Button>
          <Button
            size="sm"
            :disabled="acting || !installed"
            @click="runAction(() => api.restartService())"
          >
            <RotateCcw class="size-4" />
            Restart
          </Button>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
