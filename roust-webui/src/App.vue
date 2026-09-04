<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterLink, RouterView, useRoute } from 'vue-router'
import { api } from '@/api/client'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { AlertCircle, RefreshCw, RotateCcw } from 'lucide-vue-next'

const route = useRoute()
const refreshing = ref(false)
const restarting = ref(false)
const headerError = ref<string | null>(null)
const refreshTick = ref(0)

const nav = [
  { to: '/status', label: 'Status' },
  { to: '/routes', label: 'Routes' },
  { to: '/app-binds', label: 'App binds' },
  { to: '/ping', label: 'Ping' },
  { to: '/settings', label: 'Settings' },
  { to: '/about-me', label: 'About Me' },
] as const

const pageKey = computed(() => `${route.fullPath}:${refreshTick.value}`)

function bumpRefresh() {
  refreshTick.value += 1
}

async function refreshPage() {
  refreshing.value = true
  headerError.value = null
  try {
    await api.getStatus()
    bumpRefresh()
  } catch (e) {
    headerError.value = e instanceof Error ? e.message : String(e)
  } finally {
    refreshing.value = false
  }
}

async function restartService() {
  restarting.value = true
  headerError.value = null
  try {
    await api.restartService()
    bumpRefresh()
  } catch (e) {
    headerError.value = e instanceof Error ? e.message : String(e)
  } finally {
    restarting.value = false
  }
}

function isActive(path: string) {
  return route.path === path || route.path.startsWith(`${path}/`)
}
</script>

<template>
  <div class="min-h-screen bg-background text-foreground">
    <header class="border-b">
      <div class="mx-auto flex max-w-6xl flex-col gap-4 px-4 py-4">
        <div class="flex flex-wrap items-center justify-between gap-4">
          <div>
            <h1 class="text-xl font-semibold tracking-tight">Roust</h1>
            <p class="text-sm text-muted-foreground">
              Windows packet router — service status and route management
            </p>
          </div>
          <div class="flex flex-wrap gap-2">
            <Button
              variant="outline"
              size="sm"
              :disabled="restarting"
              @click="restartService"
            >
              <RotateCcw class="size-4" :class="{ 'animate-spin': restarting }" />
              Restart service
            </Button>
            <Button
              variant="outline"
              size="sm"
              :disabled="refreshing"
              @click="refreshPage"
            >
              <RefreshCw class="size-4" :class="{ 'animate-spin': refreshing }" />
              Refresh
            </Button>
          </div>
        </div>
        <nav class="flex flex-wrap gap-1">
          <RouterLink
            v-for="item in nav"
            :key="item.to"
            :to="item.to"
            class="rounded-md px-3 py-1.5 text-sm transition-colors"
            :class="
              isActive(item.to)
                ? 'bg-primary text-primary-foreground'
                : 'text-muted-foreground hover:bg-muted hover:text-foreground'
            "
          >
            {{ item.label }}
          </RouterLink>
        </nav>
      </div>
    </header>

    <main class="mx-auto flex max-w-6xl flex-col gap-6 px-4 py-6">
      <Alert v-if="headerError" variant="destructive">
        <AlertCircle class="size-4" />
        <AlertTitle>Action failed</AlertTitle>
        <AlertDescription>{{ headerError }}</AlertDescription>
      </Alert>
      <RouterView :key="pageKey" />
    </main>
  </div>
</template>
