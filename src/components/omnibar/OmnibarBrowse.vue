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

    <div class="main-content">
      <div class="browse-col custom-scrollbar">
        <div v-if="Object.keys(grouped).length === 0" class="empty-state">
          <v-icon icon="mdi-shape-outline" size="48" class="mb-3 opacity-50"></v-icon>
          <div class="text-h6 font-weight-regular">No categories found</div>
          <div class="text-caption text-medium-emphasis">Install some apps or rescan from Settings.</div>
        </div>

        <section
          v-for="(bucket, cat) in grouped"
          :key="cat"
          class="category-section"
        >
          <header class="category-header">
            <v-icon :icon="iconForCategory(cat)" size="18" class="mr-2 text-primary"></v-icon>
            <span class="category-title">{{ formatCategoryName(cat) }}</span>
            <v-spacer></v-spacer>
            <span class="category-count">{{ bucket.length }}</span>
          </header>

          <div
            v-for="(app, index) in bucket"
            :key="app.id"
            class="result-item glass-hover interactive browse-app-item"
            :class="{'result-item-active': isSelected(cat, index)}"
            @click="launch(app)"
            @mouseenter="setHover(cat, index)"
          >
            <div class="result-icon">
              <img v-if="app.icon" :src="convertFileSrc(app.icon)" width="24" height="24" />
              <span v-else>📦</span>
            </div>
            <div class="result-content">
              <div class="result-title">{{ app.name }}</div>
              <div class="result-subtitle text-dim">{{ app.exec }}</div>
            </div>
            <span class="source-badge" :class="`source-${app.source}`">{{ sourceLabel(app.source) }}</span>
          </div>
        </section>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, ref, watch } from 'vue'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { useOmnibar } from '../../composables/useOmnibar'

const emit = defineEmits(['close'])
const { apps, recordAction, hideWindow } = useOmnibar()

const localQuery = ref('')
const searchInput = ref(null)
const hovered = ref({ category: null, index: 0 })

const normalizedApps = computed(() => {
  const list = apps.value || []
  const q = localQuery.value.trim().toLowerCase()
  if (!q) return list
  return list.filter((a) => {
    const haystack = `${a.name || ''} ${a.exec || ''}`.toLowerCase()
    return haystack.includes(q)
  })
})

const grouped = computed(() => {
  const map = {}
  for (const app of normalizedApps.value) {
    const cats = (app && app.categories) ? app.categories : []
    if (!cats || cats.length === 0) {
      const key = 'Uncategorized'
      if (!map[key]) map[key] = []
      map[key].push(app)
      continue
    }
    for (const cat of cats) {
      const key = cat || 'Uncategorized'
      if (!map[key]) map[key] = []
      map[key].push(app)
    }
  }
  for (const key of Object.keys(map)) {
    map[key].sort((a, b) => (a.name || '').localeCompare(b.name || ''))
  }
  const ordered = {}
  for (const key of Object.keys(map).sort()) {
    ordered[key] = map[key]
  }
  return ordered
})

const flatList = computed(() => {
  const out = []
  for (const cat of Object.keys(grouped.value)) {
    grouped.value[cat].forEach((app, index) => out.push({ cat, index, app }))
  }
  return out
})

function setHover(category, index) {
  hovered.value = { category, index }
}

function isSelected(category, index) {
  if (hovered.value.category === category && hovered.value.index === index) return true
  const flatIdx = flatList.value.findIndex((e) => e.cat === category && e.index === index)
  return flatList.value[hoveredFlatIndex.value] && flatIdx === hoveredFlatIndex.value
}

const hoveredFlatIndex = computed(() => {
  if (flatList.value.length === 0) return -1
  const match = flatList.value.findIndex(
    (e) => e.cat === hovered.value.category && e.index === hovered.value.index,
  )
  if (match >= 0) return match
  return 0
})

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
  try {
    await invoke('launch_app', { execCmd: app.exec })
    recordAction(app)
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

function formatCategoryName(cat) {
  if (!cat) return 'Uncategorized'
  if (cat === 'Uncategorized') return cat
  return String(cat).replace(/[-_]/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase())
}

function sourceLabel(source) {
  if (!source) return ''
  const normalized = String(source).toLowerCase()
  return normalized.charAt(0).toUpperCase() + normalized.slice(1)
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
</style>