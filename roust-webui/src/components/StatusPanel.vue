<script setup lang="ts">
import { computed } from 'vue'
import type { StatusResponse } from '@/api/client'
import { Badge } from '@/components/ui/badge'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Activity, FileJson, Hash, Package } from 'lucide-vue-next'

const props = defineProps<{
  status: StatusResponse | null
  loading?: boolean
}>()

const stateVariant = computed(() => {
  const state = props.status?.state ?? ''
  if (state === 'Running') return 'default' as const
  if (state === 'Stopped' || state.startsWith('Error')) return 'destructive' as const
  return 'secondary' as const
})
</script>

<template>
  <Card>
    <CardHeader>
      <CardTitle class="flex items-center gap-2">
        <Activity class="size-5" />
        Service status
      </CardTitle>
      <CardDescription>
        Live state from <code class="text-xs">roust-api</code>
      </CardDescription>
    </CardHeader>
    <CardContent>
      <div v-if="loading && !status" class="text-sm text-muted-foreground">
        Loading status…
      </div>
      <div v-else-if="status" class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <div class="space-y-1">
          <p class="text-xs font-medium text-muted-foreground">State</p>
          <Badge :variant="stateVariant">{{ status.state }}</Badge>
          <p class="text-xs text-muted-foreground">
            {{ status.installed ? 'Service installed' : 'Service not installed' }}
          </p>
        </div>
        <div class="space-y-1">
          <p class="text-xs font-medium text-muted-foreground flex items-center gap-1">
            <Hash class="size-3" /> Rules
          </p>
          <p class="text-2xl font-semibold tabular-nums">{{ status.rule_count }}</p>
          <p class="text-xs text-muted-foreground">
            {{ status.config_rule_count }} app · {{ status.system_rule_count }} system
          </p>
        </div>
        <div class="space-y-1">
          <p class="text-xs font-medium text-muted-foreground flex items-center gap-1">
            <Package class="size-3" /> Version
          </p>
          <p class="font-mono text-sm">{{ status.version }}</p>
        </div>
        <div class="space-y-1 min-w-0">
          <p class="text-xs font-medium text-muted-foreground flex items-center gap-1">
            <FileJson class="size-3" /> Config
          </p>
          <p class="truncate font-mono text-xs" :title="status.config_path">
            {{ status.config_path }}
          </p>
        </div>
      </div>
    </CardContent>
  </Card>
</template>
