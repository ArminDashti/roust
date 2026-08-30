<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import {
  api,
  type AdapterItem,
  type DestinationKind,
  type RoutingRule,
  type TargetKind,
} from '@/api/client'
import { kindLabel } from '@/lib/kindLabels'
import { Button } from '@/components/ui/button'
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

const props = defineProps<{
  open: boolean
  mode: 'create' | 'edit'
  initial?: RoutingRule | null
  saving?: boolean
  error?: string | null
}>()

const emit = defineEmits<{
  'update:open': [value: boolean]
  submit: [rule: RoutingRule | RoutingRule[]]
}>()

const TARGET_KINDS: TargetKind[] = ['cidr', 'ip', 'nic', 'mac']
const DESTINATION_KINDS: DestinationKind[] = ['ip', 'nic', 'mac']

const form = reactive<{
  target: TargetKind
  'target-value': string
  destination: DestinationKind
  'destination-value': string
}>({
  target: 'cidr',
  'target-value': '',
  destination: 'ip',
  'destination-value': '',
})

const adapters = ref<AdapterItem[]>([])
const adaptersError = ref<string | null>(null)
const cidrFileLines = ref<string[]>([])
const cidrFileName = ref<string | null>(null)
const fileInputRef = ref<HTMLInputElement | null>(null)

const title = computed(() => (props.mode === 'create' ? 'Add route' : 'Edit route'))

const nicOptions = computed(() =>
  adapters.value.map((a) => ({
    value: a.display_name || a.friendly_name || a.name,
    label: a.display_name || a.friendly_name || a.name,
  })),
)

const macOptions = computed(() =>
  adapters.value
    .filter((a) => {
      const mac = a.mac_address?.trim()
      return Boolean(mac) && mac.toUpperCase() !== 'N/A'
    })
    .map((a) => {
      const nic = a.display_name || a.friendly_name || a.name
      return {
        value: a.mac_address,
        label: `${a.mac_address} (${nic})`,
      }
    }),
)

async function loadAdapters() {
  adaptersError.value = null
  try {
    adapters.value = await api.listAdapters()
  } catch (e) {
    adaptersError.value = e instanceof Error ? e.message : String(e)
    adapters.value = []
  }
}

watch(
  () => [props.open, props.initial] as const,
  async ([open]) => {
    if (!open) return
    cidrFileLines.value = []
    cidrFileName.value = null
    if (fileInputRef.value) fileInputRef.value.value = ''
    await loadAdapters()
    if (props.initial) {
      form.target = props.initial.target
      form['target-value'] = props.initial['target-value']
      form.destination = props.initial.destination
      form['destination-value'] = props.initial['destination-value']
    } else {
      form.target = 'cidr'
      form['target-value'] = ''
      form.destination = 'ip'
      form['destination-value'] = ''
    }
  },
  { immediate: true },
)

watch(
  () => form.target,
  () => {
    if (form.target !== 'cidr') {
      cidrFileLines.value = []
      cidrFileName.value = null
    }
  },
)

function onOpenChange(value: boolean) {
  emit('update:open', value)
}

function parseCidrLines(text: string): string[] {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && !line.startsWith('#'))
}

function onCidrFileChange(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  if (!file) return
  const reader = new FileReader()
  reader.onload = () => {
    const text = typeof reader.result === 'string' ? reader.result : ''
    cidrFileLines.value = parseCidrLines(text)
    cidrFileName.value = file.name
    if (cidrFileLines.value.length === 1) {
      form['target-value'] = cidrFileLines.value[0]
    } else if (cidrFileLines.value.length > 1) {
      form['target-value'] = ''
    }
  }
  reader.readAsText(file)
}

function onSubmit() {
  const destination = form.destination
  const destinationValue = form['destination-value'].trim()

  if (props.mode === 'create' && form.target === 'cidr' && cidrFileLines.value.length > 1) {
    const rules: RoutingRule[] = cidrFileLines.value.map((cidr) => ({
      target: 'cidr',
      'target-value': cidr,
      destination,
      'destination-value': destinationValue,
    }))
    emit('submit', rules)
    return
  }

  const targetValue =
    form.target === 'cidr' && cidrFileLines.value.length === 1
      ? cidrFileLines.value[0]
      : form['target-value'].trim()

  emit('submit', {
    target: form.target,
    'target-value': targetValue,
    destination,
    'destination-value': destinationValue,
  })
}

onMounted(() => {
  if (props.open) void loadAdapters()
})
</script>

<template>
  <Dialog :open="open" @update:open="onOpenChange">
    <DialogContent class="sm:max-w-lg">
      <DialogHeader>
        <DialogTitle>{{ title }}</DialogTitle>
        <DialogDescription>
          Target selects traffic; destination is where matched packets are sent.
          Saving restarts the Roust service when it is running.
        </DialogDescription>
      </DialogHeader>

      <form class="grid gap-4 py-2" @submit.prevent="onSubmit">
        <div class="grid gap-2">
          <Label for="target">Target kind</Label>
          <Select v-model="form.target">
            <SelectTrigger id="target" class="w-full">
              <SelectValue placeholder="Select target" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem v-for="kind in TARGET_KINDS" :key="kind" :value="kind">
                {{ kindLabel(kind) }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div class="grid gap-2">
          <Label for="target-value">Target value</Label>

          <template v-if="form.target === 'cidr'">
            <Input
              id="target-value"
              v-model="form['target-value']"
              placeholder="e.g. 10.0.0.0/8"
              :required="cidrFileLines.length === 0"
              autocomplete="off"
              :disabled="cidrFileLines.length > 1"
            />
            <div class="space-y-1">
              <Label for="cidr-file" class="text-xs text-muted-foreground">
                Or upload a .txt file (one CIDR per line)
              </Label>
              <Input
                id="cidr-file"
                ref="fileInputRef"
                type="file"
                accept=".txt,text/plain"
                class="cursor-pointer"
                @change="onCidrFileChange"
              />
              <p
                v-if="cidrFileName"
                class="text-xs text-muted-foreground"
              >
                {{ cidrFileName }} — {{ cidrFileLines.length }} CIDR
                {{ cidrFileLines.length === 1 ? 'entry' : 'entries' }}
              </p>
            </div>
          </template>

          <Input
            v-else-if="form.target === 'ip'"
            id="target-value"
            v-model="form['target-value']"
            placeholder="e.g. 8.8.8.8"
            required
            autocomplete="off"
          />

          <Select
            v-else-if="form.target === 'nic'"
            v-model="form['target-value']"
          >
            <SelectTrigger id="target-value" class="w-full">
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

          <Select
            v-else-if="form.target === 'mac'"
            v-model="form['target-value']"
          >
            <SelectTrigger id="target-value" class="w-full">
              <SelectValue placeholder="Select MAC" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="opt in macOptions"
                :key="opt.value"
                :value="opt.value"
              >
                {{ opt.label }}
              </SelectItem>
            </SelectContent>
          </Select>

          <p v-if="adaptersError && (form.target === 'nic' || form.target === 'mac')" class="text-xs text-destructive">
            {{ adaptersError }}
          </p>
        </div>

        <div class="grid gap-2">
          <Label for="destination">Destination kind</Label>
          <Select v-model="form.destination">
            <SelectTrigger id="destination" class="w-full">
              <SelectValue placeholder="Select destination" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="kind in DESTINATION_KINDS"
                :key="kind"
                :value="kind"
              >
                {{ kindLabel(kind) }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div class="grid gap-2">
          <Label for="destination-value">Destination value</Label>

          <Input
            v-if="form.destination === 'ip'"
            id="destination-value"
            v-model="form['destination-value']"
            placeholder="e.g. 192.168.1.1"
            required
            autocomplete="off"
          />

          <Select
            v-else-if="form.destination === 'nic'"
            v-model="form['destination-value']"
          >
            <SelectTrigger id="destination-value" class="w-full">
              <SelectValue placeholder="Select NIC" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="opt in nicOptions"
                :key="`dst-nic-${opt.value}`"
                :value="opt.value"
              >
                {{ opt.label }}
              </SelectItem>
            </SelectContent>
          </Select>

          <Select
            v-else-if="form.destination === 'mac'"
            v-model="form['destination-value']"
          >
            <SelectTrigger id="destination-value" class="w-full">
              <SelectValue placeholder="Select MAC" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="opt in macOptions"
                :key="`dst-mac-${opt.value}`"
                :value="opt.value"
              >
                {{ opt.label }}
              </SelectItem>
            </SelectContent>
          </Select>

          <p
            v-if="adaptersError && (form.destination === 'nic' || form.destination === 'mac')"
            class="text-xs text-destructive"
          >
            {{ adaptersError }}
          </p>
        </div>

        <p v-if="error" class="text-sm text-destructive">{{ error }}</p>

        <DialogFooter class="gap-2 sm:gap-0">
          <Button type="button" variant="outline" @click="onOpenChange(false)">
            Cancel
          </Button>
          <Button type="submit" :disabled="saving">
            {{
              saving
                ? 'Saving…'
                : mode === 'create'
                  ? cidrFileLines.length > 1
                    ? `Add ${cidrFileLines.length} routes`
                    : 'Add route'
                  : 'Save changes'
            }}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
