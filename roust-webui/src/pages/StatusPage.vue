<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { api, type StatusResponse } from '@/api/client'
import StatusPanel from '@/components/StatusPanel.vue'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { AlertCircle } from 'lucide-vue-next'

const status = ref<StatusResponse | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)

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

onMounted(refresh)
</script>

<template>
  <div class="flex flex-col gap-6">
    <Alert v-if="error" variant="destructive">
      <AlertCircle class="size-4" />
      <AlertTitle>Could not reach API</AlertTitle>
      <AlertDescription>{{ error }}</AlertDescription>
    </Alert>
    <StatusPanel :status="status" :loading="loading" />
  </div>
</template>
