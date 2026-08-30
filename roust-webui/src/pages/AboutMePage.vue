<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { User } from 'lucide-vue-next'

const parts = {
  user: 'arminonline71',
  at: '@',
  domain: 'gmail',
  dot: '.',
  tld: 'com',
}

const mailtoHref = ref('#')
const photoFailed = ref(false)

onMounted(() => {
  mailtoHref.value = `mailto:${parts.user}${parts.at}${parts.domain}${parts.dot}${parts.tld}`
})

const base = import.meta.env.BASE_URL || '/'
const photoSrc = `${base.endsWith('/') ? base : `${base}/`}about-me/armin.png`
</script>

<template>
  <div class="mx-auto flex max-w-3xl flex-col gap-6">
    <header class="space-y-1">
      <h2 class="flex items-center gap-2 text-2xl font-semibold tracking-tight">
        <User class="size-6" />
        About Me
      </h2>
      <p class="text-sm text-muted-foreground">
        Armin Dashti — vibe coder, conductor of craft.
      </p>
    </header>

    <div class="grid gap-6 sm:grid-cols-[160px_1fr] sm:items-start">
      <div class="overflow-hidden rounded-2xl border bg-muted/40">
        <img
          v-if="!photoFailed"
          :src="photoSrc"
          alt="Armin Dashti"
          class="aspect-[4/5] w-full object-cover"
          @error="photoFailed = true"
        />
        <div
          v-else
          class="flex aspect-[4/5] items-center justify-center px-3 text-center text-xs text-muted-foreground"
        >
          Photo placeholder — add public/about-me/armin.png
        </div>
      </div>

      <div class="space-y-4 text-[15px] leading-relaxed">
        <p>
          Armin Dashti is a software engineer and vibe coder. He shapes intent with clarity and
          constraint, then lets Cursor’s agents compose the implementation; he reviews, refines,
          and ships what remains worthy.
        </p>
        <p>
          This application was written by AI agents—specifically Cursor—guided by Armin’s intent,
          then tempered by his review until fit to ship.
        </p>
        <p>
          He builds practical API and WebUI products with substance over spectacle: a Rust
          management API and Vue front end for Windows packet routing. He watches carefully, plans
          deliberately, and ships when ready.
        </p>
      </div>
    </div>

    <section class="space-y-2 rounded-xl border p-4">
      <h3 class="text-sm font-semibold uppercase tracking-wide text-muted-foreground">
        Interests
      </h3>
      <p class="text-sm leading-relaxed">
        Cars, movies, vibe coding, coding, politics, military, jet fighters, and aircraft.
      </p>
    </section>

    <p class="text-sm">
      Contact:
      <a :href="mailtoHref" class="underline-offset-2 hover:underline">
        {{ parts.user }} [at] {{ parts.domain }} [dot] {{ parts.tld }}
      </a>
    </p>

    <div class="space-y-3">
      <blockquote class="border-s-2 ps-4 text-sm italic leading-relaxed text-muted-foreground">
        "Beware the quiet man. For while others speak, he watches. While others act, he plans. And
        when they finally rest, he strikes." — Anonymous
      </blockquote>
      <blockquote
        dir="rtl"
        lang="fa"
        class="border-s-2 ps-4 text-sm italic leading-relaxed text-muted-foreground"
      >
        «از مرد خاموش برحذر باش. زیرا در حالی که دیگران سخن می‌گویند، او نظاره می‌کند. در حالی که
        دیگران عمل می‌کنند، او برنامه می‌ریزد. و هنگامی که سرانجام آرام می‌گیرند، او ضربه
        می‌زند.» — ناشناس
      </blockquote>
    </div>
  </div>
</template>
