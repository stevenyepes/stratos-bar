<template>
  <div class="omnibar-browse-mode scale-in">
    <div data-tauri-drag-region class="drag-handle"></div>

    <div class="search-container">
      <div class="browse-mode-badge scale-in">BROWSE</div>
      <input
        ref="searchInput"
        v-model="localQuery"
        type="text"
        placeholder="🔍 Filter apps by name..."
        class="search-input font-primary"
        @keydown.down.prevent="navigateResults(1)"
        @keydown.up.prevent="navigateResults(-1)"
        @keydown.enter.prevent="activateSelected"
        @keydown.esc.stop="emit('close')"
      />
      <button class="back-btn interactive" @click="emit('close')" title="Back to search">←</button>
    </div>

    <div
      v-if="hasChips"
      class="chip-strip custom-scrollbar"
      data-testid="browse-chip-strip"
    >
      <div v-if="categoriesWithCounts.length > 0" class="chip-row">
        <span class="chip-row-label">Category</span>
        <button
          class="chip interactive"
          :class="{ 'chip-active': activeCategory === null }"
          data-testid="chip-category-all"
          @click="setCategory(null)"
        >
          All
          <span class="chip-count">{{ totalCount }}</span>
        </button>
        <button
          v-for="cat in categoriesWithCounts"
          :key="`cat-${cat.name}`"
          class="chip interactive"
          :class="{ 'chip-active': activeCategory === cat.name }"
          :data-testid="`chip-category-${cat.name}`"
          @click="setCategory(cat.name)"
        >
          {{ displayCategoryName(cat.name) }}
          <span class="chip-count">{{ cat.count }}</span>
        </button>
      </div>

      <div v-if="sourcesWithCounts.length > 1" class="chip-row">
        <span class="chip-row-label">Source</span>
        <button
          class="chip interactive"
          :class="{ 'chip-active': activeSource === null }"
          data-testid="chip-source-all"
          @click="setSource(null)"
        >
          All
        </button>
        <button
          v-for="src in sourcesWithCounts"
          :key="`src-${src.name}`"
          class="chip interactive"
          :class="{
            'chip-active': activeSource === src.name,
            [`source-${src.name}`]: true,
          }"
          :data-testid="`chip-source-${src.name}`"
          @click="setSource(src.name)"
        >
          {{ src.label }}
          <span class="chip-count">{{ src.count }}</span>
        </button>
      </div>
    </div>

    <div class="main-content">
      <div class="browse-col custom-scrollbar">
        <div v-if="Object.keys(filteredGrouped).length === 0" class="empty-state">
          <v-icon icon="mdi-shape-outline" size="48" class="mb-3 opacity-50"></v-icon>
          <div class="text-h6 font-weight-regular">No categories found</div>
          <div class="text-caption text-medium-emphasis">Install some apps or rescan from Settings.</div>
        </div>

        <section
          v-for="(bucket, cat) in filteredGrouped"
          :key="cat"
          class="category-section"
          :data-testid="`category-section-${cat}`"
        >
          <header class="category-header">
            <v-icon :icon="iconForCategory(cat)" size="18" class="mr-2 text-primary"></v-icon>
            <span class="category-title">{{ displayCategoryName(cat) }}</span>
            <v-spacer></v-spacer>
            <span class="category-count">{{ bucket.length }}</span>
          </header>

          <div
            v-for="(app, index) in bucket"
            :key="app.id"
            class="result-item glass-hover interactive browse-app-item"
            :class="{ 'result-item-active': isSelected(cat, index) }"
            :data-testid="`browse-app-${app.id}`"
            @click="launch(app)"
            @mouseenter="setHover(cat, index)"
          >
            <div class="result-icon">
              <img v-if="app.icon" :src="convertFileSrc(app.icon)" width="24" height="24" />
              <span v-else>📦</span>
            </div>
            <div class="result-content">
              <div class="result-title">{{ app.name }}</div>
              <div class="result-subtitle text-dim">{{ launchExec(app) }}</div>
            </div>
            <span class="source-badge" :class="`source-${sourceKey(app.source)}`">
              {{ sourceLabel(app.source) }}
            </span>
          </div>
        </section>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, onMounted, ref, watch } from 'vue'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { useOmnibar } from '../../composables/useOmnibar'

const emit = defineEmits(['close'])
const { recordAction, hideWindow } = useOmnibar()

const localQuery = ref('')
const searchInput = ref(null)
const hovered = ref({ category: null, index: 0 })

const sections = ref({
  apps_by_category: {},
  scripts: [],
  ai_tools: [],
  recent_files: [],
  shortcuts: {},
  category_counts: {},
  source_counts: {},
  kind_counts: {},
})

const activeCategory = ref(null)
const activeSource = ref(null)

async function loadBrowse() {
  try {
    const result = await invoke('browse_discoverable')
    sections.value = result || sections.value
  } catch (e) {
    console.error('browse_discoverable failed', e)
  }
}

onMounted(() => {
  loadBrowse()
})

function sourceKey(source) {
  if (!source) return ''
  const s = String(source)
  const idx = s.indexOf(':')
  return idx >= 0 ? s.slice(idx + 1) : s
}

function sourceLabel(source) {
  const key = sourceKey(source)
  if (!key) return ''
  return key.charAt(0).toUpperCase() + key.slice(1)
}

function displayCategoryName(cat) {
  if (!cat || cat === 'Other') return 'Uncategorized'
  return String(cat).replace(/[-_]/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase())
}

function launchExec(app) {
  if (app && app.launch && typeof app.launch.exec === 'string') return app.launch.exec
  return ''
}

const categoriesWithCounts = computed(() => {
  const counts = sections.value.category_counts || {}
  return Object.keys(counts)
    .filter((k) => k)
    .sort()
    .map((k) => ({ name: k, count: counts[k] || 0 }))
})

const sourcesWithCounts = computed(() => {
  const counts = sections.value.source_counts || {}
  const seen = new Map()
  for (const raw of Object.keys(counts)) {
    const key = sourceKey(raw)
    if (!key) continue
    if (!seen.has(key)) {
      seen.set(key, { name: key, label: sourceLabel(raw), count: 0 })
    }
    seen.get(key).count += counts[raw] || 0
  }
  return Array.from(seen.values()).sort((a, b) => a.name.localeCompare(b.name))
})

const hasChips = computed(
  () => categoriesWithCounts.value.length > 0 || sourcesWithCounts.value.length > 0,
)

const totalCount = computed(() =>
  Object.values(sections.value.category_counts || {}).reduce((a, b) => a + b, 0),
)

function setCategory(name) {
  activeCategory.value = activeCategory.value === name ? null : name
  hovered.value = { category: null, index: 0 }
}

function setSource(name) {
  activeSource.value = activeSource.value === name ? null : name
  hovered.value = { category: null, index: 0 }
}

const filteredGrouped = computed(() => {
  const map = sections.value.apps_by_category || {}
  const out = {}
  const q = localQuery.value.trim().toLowerCase()
  const cat = activeCategory.value
  const src = activeSource.value

  for (const key of Object.keys(map)) {
    if (cat && key !== cat) continue
    const bucket = (map[key] || []).filter((app) => {
      if (src && sourceKey(app.source) !== src) return false
      if (q) {
        const haystack = `${app.name || ''} ${launchExec(app)}`.toLowerCase()
        if (!haystack.includes(q)) return false
      }
      return true
    })
    if (bucket.length > 0) out[key] = bucket
  }
  return out
})

const flatList = computed(() => {
  const out = []
  for (const cat of Object.keys(filteredGrouped.value)) {
    filteredGrouped.value[cat].forEach((app, index) => out.push({ cat, index, app }))
  }
  return out
})

const hoveredFlatIndex = computed(() => {
  if (flatList.value.length === 0) return -1
  const match = flatList.value.findIndex(
    (e) => e.cat === hovered.value.category && e.index === hovered.value.index,
  )
  if (match >= 0) return match
  return 0
})

function setHover(category, index) {
  hovered.value = { category, index }
}

function isSelected(category, index) {
  if (hovered.value.category === category && hovered.value.index === index) return true
  const flatIdx = flatList.value.findIndex((e) => e.cat === category && e.index === index)
  return flatList.value[hoveredFlatIndex.value] && flatIdx === hoveredFlatIndex.value
}

function navigateResults(direction) {
  if (flatList.value.length === 0) return
  const cur = hoveredFlatIndex.value >= 0 ? hoveredFlatIndex.value : 0
  const next = (cur + direction + flatList.value.length) % flatList.value.length
  const entry = flatList.value[next]
  if (entry) {
    hovered.value = { category: entry.cat, index: entry.index }
  }
}

async function activateSelected() {
  if (flatList.value.length === 0) return
  const idx = hoveredFlatIndex.value >= 0 ? hoveredFlatIndex.value : 0
  const entry = flatList.value[idx]
  if (entry) await launch(entry.app)
}

async function launch(app) {
  const exec = launchExec(app)
  if (!exec) return
  try {
    await invoke('launch_app', { execCmd: exec })
    recordAction({
      id: app.id,
      name: app.name,
      exec,
      icon: app.icon || null,
      source: sourceKey(app.source),
    })
    await hideWindow()
  } catch (e) {
    console.error('Failed to launch app', e)
  }
}

function iconForCategory(cat) {
  const lower = String(cat || '').toLowerCase()
  if (lower.includes('net')) return 'mdi-web'
  if (lower.includes('dev')) return 'mdi-code-braces'
  if (lower.includes('audio') || lower.includes('video') || lower.includes('multi')) return 'mdi-movie-open'
  if (lower.includes('graphic') || lower.includes('art')) return 'mdi-palette-outline'
  if (lower.includes('office') || lower.includes('text')) return 'mdi-file-document-outline'
  if (lower.includes('game')) return 'mdi-gamepad-variant-outline'
  if (lower.includes('system')) return 'mdi-cog-outline'
  if (lower.includes('science') || lower.includes('edu')) return 'mdi-flask-outline'
  if (lower.includes('utility')) return 'mdi-tools'
  if (lower.includes('settings')) return 'mdi-tune'
  if (lower.includes('communication') || lower.includes('chat')) return 'mdi-chat-outline'
  if (lower.includes('file')) return 'mdi-folder-outline'
  return 'mdi-shape-outline'
}

watch(localQuery, () => {
  hovered.value = { category: null, index: 0 }
})
</script>

<style scoped>
.omnibar-browse-mode {
  width: 100%;
  height: 100%;
  background: var(--theme-background);
  box-shadow: var(--shadow-xl);
  border: 1px solid var(--theme-border);
  display: flex;
  flex-direction: column;
  backdrop-filter: blur(20px);
}

.drag-handle {
  height: 24px;
  width: 100%;
  cursor: move;
  flex-shrink: 0;
}

.search-container {
  padding: 0 var(--space-6);
  padding-bottom: var(--space-4);
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.browse-mode-badge {
  flex-shrink: 0;
  padding: 2px 8px;
  border-radius: var(--radius-sm);
  background: rgba(122, 162, 247, 0.2);
  color: var(--theme-primary);
  font-size: var(--font-size-xs);
  font-weight: var(--font-weight-semibold);
  border: 1px solid rgba(122, 162, 247, 0.3);
  white-space: nowrap;
}

.search-input {
  width: 100%;
  background: transparent;
  border: none;
  outline: none;
  font-size: var(--font-size-lg);
  color: var(--theme-text);
  padding: var(--space-2) 0;
}

.search-input::placeholder {
  color: var(--theme-text-dimmer);
}

.back-btn {
  flex-shrink: 0;
  background: transparent;
  border: 1px solid var(--theme-border);
  color: var(--theme-text-dim);
  border-radius: var(--radius-md);
  width: 32px;
  height: 32px;
  cursor: pointer;
  font-size: 16px;
}

.back-btn:hover {
  background: rgba(255, 255, 255, 0.05);
  color: var(--theme-text);
}

.chip-strip {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: 0 var(--space-6) var(--space-3);
  border-bottom: 1px solid var(--theme-border);
  flex-shrink: 0;
  overflow-x: auto;
}

.chip-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: nowrap;
}

.chip-row-label {
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--theme-text-dimmer);
  font-weight: var(--font-weight-semibold);
  flex-shrink: 0;
  width: 56px;
}

.chip {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: 4px 10px;
  font-size: var(--font-size-xs);
  font-weight: var(--font-weight-medium);
  border-radius: var(--radius-full);
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid var(--theme-border);
  color: var(--theme-text-dim);
  cursor: pointer;
  white-space: nowrap;
  transition: all var(--duration-fast) var(--ease-out);
}

.chip:hover {
  background: rgba(255, 255, 255, 0.08);
  color: var(--theme-text);
}

.chip-active {
  background: rgba(122, 162, 247, 0.18);
  border-color: rgba(122, 162, 247, 0.45);
  color: var(--theme-primary);
}

.chip-count {
  font-size: 10px;
  background: rgba(255, 255, 255, 0.08);
  padding: 0 6px;
  border-radius: 8px;
  line-height: 1.4;
}

.chip-active .chip-count {
  background: rgba(122, 162, 247, 0.25);
}

.main-content {
  flex: 1;
  min-height: 0;
  display: flex;
  overflow: hidden;
}

.browse-col {
  flex: 1;
  overflow-y: auto;
  padding: 0 var(--space-4) var(--space-4);
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--space-12) var(--space-6);
  text-align: center;
  color: var(--theme-text-dim);
}

.category-section {
  margin-bottom: var(--space-5);
}

.category-header {
  display: flex;
  align-items: center;
  padding: var(--space-2) var(--space-4);
  font-size: var(--font-size-xs);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--theme-text-dimmer);
  font-weight: var(--font-weight-semibold);
}

.category-title {
  font-weight: 700;
}

.category-count {
  font-size: 10px;
  background: rgba(255, 255, 255, 0.05);
  padding: 1px 8px;
  border-radius: 8px;
}

.result-item {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-4);
  border-radius: var(--radius-lg);
  margin-bottom: var(--space-1);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
  background: rgba(255, 255, 255, 0.02);
  border-left: 2px solid transparent;
}

.result-item:hover {
  background: rgba(255, 255, 255, 0.05);
}

.result-item-active {
  background: rgba(122, 162, 247, 0.15) !important;
  border-left: 2px solid var(--theme-primary);
  box-shadow: inset 10px 0 20px -10px rgba(122, 162, 247, 0.2);
}

.result-icon {
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 20px;
  flex-shrink: 0;
}

.result-content {
  flex: 1;
  min-width: 0;
}

.result-title {
  font-size: var(--font-size-sm);
  font-weight: var(--font-weight-medium);
  color: var(--theme-text);
  margin-bottom: 2px;
}

.result-subtitle {
  font-size: var(--font-size-xs);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.source-badge {
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  padding: 2px 6px;
  border-radius: 4px;
  flex-shrink: 0;
  background: rgba(255, 255, 255, 0.05);
  color: var(--theme-text-dim, rgba(255, 255, 255, 0.7));
  border: 1px solid rgba(255, 255, 255, 0.08);
}

.source-badge.source-flatpak {
  background: rgba(122, 162, 247, 0.15);
  color: #7aa2f7;
  border-color: rgba(122, 162, 247, 0.3);
}

.source-badge.source-snap {
  background: rgba(247, 122, 162, 0.15);
  color: #f77aa2;
  border-color: rgba(247, 122, 162, 0.3);
}

.source-badge.source-appimage {
  background: rgba(247, 200, 122, 0.15);
  color: #f7c87a;
  border-color: rgba(247, 200, 122, 0.3);
}

.source-badge.source-nix {
  background: rgba(122, 247, 162, 0.15);
  color: #7af7a2;
  border-color: rgba(122, 247, 162, 0.3);
}

.source-badge.source-desktop {
  background: rgba(187, 154, 247, 0.15);
  color: #bb9af7;
  border-color: rgba(187, 154, 247, 0.3);
}
</style>