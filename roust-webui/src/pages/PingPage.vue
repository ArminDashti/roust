<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { api, type AdapterItem, type PingResult } from '@/api/client'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
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
import { AlertCircle, Activity } from 'lucide-vue-next'

const host = ref('192.168.0.254')
const nic = ref('')
const count = ref(4)
const adapters = ref<AdapterItem[]>([])
const adaptersError = ref<string | null>(null)
const loadingAdapters = ref(false)
const pinging = ref(false)
const error = ref<string | null>(null)
const result = ref<PingResult | null>(null)

const nicOptions = computed(() =>
  adapters.value
    .filter((a) => {
      const ip = a.ipv4_address?.trim()
      return Boolean(ip) && ip !== '0.0.0.0'
    })
    .map((a) => {
      const label = a.display_name || a.friendly_name || a.name
      const ip = a.ipv4_address ?? ''
      return {
        value: label,
        label: ip ? `${label} (${ip})` : label,
      }
    }),
)

async function loadAdapters() {
  loadingAdapters.value = true
  adaptersError.value = null
  try {
    adapters.value = await api.listAdapters()
    if (!nic.value && nicOptions.value.length > 0) {
      nic.value = nicOptions.value[0].value
    }
  } catch (e) {
    adaptersError.value = e instanceof Error ? e.message : String(e)
    adapters.value = []
  } finally {
    loadingAdapters.value = false
  }
}

async function runPing() {
  error.value = null
  result.value = null
  const hostValue = host.value.trim()
  const nicValue = nic.value.trim()
  if (!hostValue) {
    error.value = 'Enter a host IP or name'
    return
  }
  if (!nicValue) {
    error.value = 'Select a NIC'
    return
  }

  pinging.value = true
  try {
    result.value = await api.ping({
      host: hostValue,
      nic: nicValue,
      count: Math.min(10, Math.max(1, Number(count.value) || 4)),
    })
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    pinging.value = false
  }
}

onMounted(loadAdapters)
</script>

<template>
  <div class="flex flex-col gap-6">
    <Alert v-if="error" variant="destructive">
      <AlertCircle class="size-4" />
      <AlertTitle>Ping failed</AlertTitle>
      <AlertDescription>{{ error }}</AlertDescription>
    </Alert>

    <Card>
      <CardHeader>
        <CardTitle class="flex items-center gap-2">
          <Activity class="size-5" />
          Ping from NIC
        </CardTitle>
        <CardDescription>
          Temporary diagnostic: forces egress via the selected NIC for this host,
          then restores the previous Windows routes. Does not change saved Roust
          rules.
        </CardDescription>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="grid gap-4 sm:grid-cols-2">
          <div class="grid gap-2">
            <Label for="ping-host">Host</Label>
            <Input
              id="ping-host"
              v-model="host"
              placeholder="e.g. 192.168.0.254"
              autocomplete="off"
              :disabled="pinging"
            />
          </div>
          <div class="grid gap-2">
            <Label for="ping-nic">NIC</Label>
            <Select v-model="nic" :disabled="pinging || loadingAdapters">
              <SelectTrigger id="ping-nic" class="w-full">
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
            <p v-if="adaptersError" class="text-xs text-destructive">
              {{ adaptersError }}
            </p>
          </div>
          <div class="grid gap-2">
            <Label for="ping-count">Count (1–10)</Label>
            <Input
              id="ping-count"
              v-model.number="count"
              type="number"
              min="1"
              max="10"
              :disabled="pinging"
            />
          </div>
        </div>

        <Button :disabled="pinging || loadingAdapters" @click="runPing">
          {{ pinging ? 'Pinging…' : 'Ping' }}
        </Button>
      </CardContent>
    </Card>

    <Card v-if="result">
      <CardHeader>
        <CardTitle class="text-base">Results</CardTitle>
        <CardDescription>
          {{ result.received }}/{{ result.sent }} received ·
          {{ result.loss_pct.toFixed(0) }}% loss · source
          {{ result.source_ip }} → {{ result.dest_ip }} via {{ result.nic }}
          <span v-if="result.route_restored"> · routes restored</span>
        </CardDescription>
      </CardHeader>
      <CardContent>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Seq</TableHead>
              <TableHead>Status</TableHead>
              <TableHead>RTT</TableHead>
              <TableHead>Detail</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="reply in result.replies" :key="reply.seq">
              <TableCell>{{ reply.seq }}</TableCell>
              <TableCell>{{ reply.success ? 'ok' : 'fail' }}</TableCell>
              <TableCell>
                {{ reply.rtt_ms != null ? `${reply.rtt_ms} ms` : '—' }}
              </TableCell>
              <TableCell class="text-muted-foreground">
                {{ reply.error ?? '' }}
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  </div>
</template>
