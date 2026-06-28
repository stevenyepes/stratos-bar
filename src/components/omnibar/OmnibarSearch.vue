<template>
  <div class="omnibar-search-mode scale-in">
    <!-- Drag handle -->
    <div data-tauri-drag-region class="drag-handle"></div>
    
    <!-- Search input -->
    <div class="search-container">
      <div v-if="isFileSearchMode" class="search-mode-badge scale-in">FILE SEARCH</div>
      <input
        ref="searchInput"
        v-model="query"
        type="text"
        :placeholder="uiState === 'idle' ? '🔍 Type a command, search files, or ask AI...' : 'Type to search...'"
        class="search-input font-primary"
        @keydown.down.prevent="navigateResults(1)"
        @keydown.up.prevent="navigateResults(-1)"
        @keydown.enter.prevent="executeAction(selectedIndex)"
        @keydown.esc.stop="handleEsc"
        @keydown.ctrl.n.prevent="askAI"
        @keydown.ctrl.0.prevent="handleKindShortcut('all')"
        @keydown.ctrl.1.prevent="handleKindShortcut('apps')"
        @keydown.ctrl.2.prevent="handleKindShortcut('scripts')"
        @keydown.ctrl.3.prevent="handleKindShortcut('files')"
        @keydown.ctrl.4.prevent="handleKindShortcut('recent')"
      />
    </div>

    <!-- Kind Filter Chips -->
    <div
      v-if="query && kindFilterOptions.length > 0"
      class="kind-filter-row custom-scrollbar"
      data-testid="kind-filter-row"
    >
      <button
        v-for="option in kindFilterOptions"
        :key="option.id"
        type="button"
        class="kind-chip interactive"
        :class="{ 'kind-chip-active': isKindFilterActive(option.id) }"
        :data-testid="`kind-chip-${option.id}`"
        @click="setKindFilter(option.id)"
      >
        <span class="kind-chip-label">{{ option.label }}</span>
        <span v-if="shortcutFor(option.id)" class="kind-chip-hint text-dimmer">[{{ shortcutFor(option.id) }}]</span>
      </button>
    </div>

    <!-- Category Filter Bar (Development / Internet / etc.) -->
    <CategoryFilterBar
      v-if="categoryOptions.length > 0"
      :options="categoryOptions"
      :active="activeCategory"
      @select="onCategorySelect"
    />

    <!-- Top Apps rail (Raycast-style idle suggestions) -->
    <TopAppsRail
      v-if="!query"
      :top-apps="topApps"
      @launch="onTopAppLaunch"
    />

    <!-- Main Content Area -->
    <div class="main-content">
        <!-- Results Column -->
        <div class="results-col custom-scrollbar">
            <!-- Empty state (Default Items) -->
            <div v-if="!query" class="results-section">
                 <div class="section-header">SUGGESTED</div>
                 <div 
                   class="result-item glass-hover interactive"
                   :class="{'result-item-active': selectedIndex === 0}"
                   @click="showSettings = true"
                 >
                   <div class="result-icon">⚙️</div>
                   <div class="result-content">
                     <div class="result-title">Settings</div>
                     <div class="result-subtitle text-dim">Configure appearance, shortcuts, and AI</div>
                   </div>
                   <div class="result-hint text-dimmer">[↵]</div>
                 </div>
            </div>
            <!-- Recent Actions -->
            <div v-if="!query && recentActions.length > 0" class="results-section">
                 <div class="section-header d-flex align-center justify-space-between">
                    <span>RECENT ACTIONS</span>
                    <button class="clear-btn" @click.stop="handleClear">CLEAR</button>
                 </div>
                 <div
                   v-for="(action, index) in recentActions"
                   :key="action.id"
                   class="result-item glass-hover interactive"
                   :class="{'result-item-active': selectedIndex === (index + 1)}"
                   @click="executeAction(index + 1)"
                 >
                   <div class="result-icon">
                       <img v-if="action.icon && action.kind === 'app'" :src="convertFileSrc(action.icon)" width="24" height="24" />
                       <span v-else-if="action.kind === 'app'">🚀</span>
                       <span v-else-if="action.kind === 'script'">💻</span>
                       <span v-else-if="action.kind === 'ai'">🤖</span>
                       <span v-else>📄</span>
                   </div>
                   <div class="result-content">
                     <div class="result-title">{{ action.name }}</div>
                     <div class="result-subtitle text-dim">{{ action.content }}</div>
                   </div>
                   <div class="result-hint text-dimmer" v-if="selectedIndex === (index + 1)">[↵]</div>
                 </div>
            </div>

            <!-- Results -->
            <div v-else>
              <!-- AI Actions Section -->
              <div v-if="matchedTool || query" class="results-section">
                <!-- ... AI items same as before ... -->
                <div class="section-header">{{ topSectionHeader }}</div>
                
                <CurrencyResult 
                  v-if="matchedTool && matchedTool.data && matchedTool.data.type === 'currency'"
                  :data="matchedTool.data"
                  @execute="executeAction(0)"
                  @swap="swapCurrencyQuery(matchedTool.data)"
                />

                <div 
                  v-else-if="matchedTool"
                  class="result-item ai-action-item interactive"
                  :class="{'result-item-active': selectedIndex === 0}"
                  @click="executeAction(0)"
                >
                  <div class="result-icon ai-icon-glow">{{ matchedTool.icon || '✨' }}</div>
                  <div class="result-content">
                    <div class="result-title text-gradient">{{ matchedTool.name }}</div>
                    <div class="result-subtitle text-dim">{{ matchedTool.description }}</div>
                  </div>
                  <div class="result-hint text-dimmer">[↵]</div>
                </div>

                <div 
                  v-else
                  class="result-item ai-action-item interactive"
                  :class="{'result-item-active': selectedIndex === 0}"
                  @click="askAI()"
                >
                  <div class="result-icon ai-icon-glow">🤖</div>
                  <div class="result-content">
                    <div class="result-title text-gradient">Ask AI: "{{ query }}"</div>
                    <div class="result-subtitle text-dim">Get instant answers</div>
                  </div>
                  <div class="result-hint text-dimmer"><span class="mr-2 text-xs opacity-70">[Ctrl+N]</span> [↵]</div>
                </div>
              </div>

              <!-- Open Windows -->
              <div v-if="filteredWindows.length" class="results-section">
                <div class="section-header">OPEN APPS</div>
                <div
                  v-for="(win, index) in filteredWindows"
                  :key="'win-'+index"
                  class="result-item glass-hover interactive"
                  :class="{'result-item-active': selectedIndex === (1 + index)}"
                  @click="focusWindow(win)"
                >
                  <div class="result-icon">
                    <img v-if="win.icon" :src="convertFileSrc(win.icon)" width="24" height="24" />
                    <span v-else>🔲</span>
                  </div>
                  <div class="result-content">
                    <div class="result-title" v-html="highlightMatch(win.title)"></div>
                    <div class="result-subtitle text-dim">Switch to {{ win.class }}</div>
                  </div>
                </div>
              </div>

              <!-- Applications -->
              <div v-if="appsByCategory.length" class="results-section">
                <div class="section-header">APPLICATIONS</div>
                <div
                  v-for="(app, index) in appsByCategory"
                  :key="'app-'+index"
                  class="result-item glass-hover interactive"
                  :class="{'result-item-active': selectedIndex === (1 + filteredWindows.length + index)}"
                  @click="executeApp(app)"
                >
                  <div class="result-icon">
                    <img v-if="app.icon" :src="convertFileSrc(app.icon)" width="24" height="24" />
                    <span v-else>📦</span>
                  </div>
                  <div class="result-content">
                    <div class="result-title" v-html="highlightMatch(app.name)"></div>
                    <div class="result-subtitle text-dim">{{ app.exec }}</div>
                  </div>
                  <span class="source-badge" :class="`source-${app.source}`">{{ sourceLabel(app.source) }}</span>
                  <span v-if="appScoreFor(app) !== null" class="score-hint text-dimmer">{{ formatScore(appScoreFor(app)) }}</span>
                </div>
              </div>

              <!-- Scripts -->
              <div v-if="filteredScripts.length" class="results-section">
                <div class="section-header">SCRIPTS</div>
                <div
                  v-for="(script, index) in filteredScripts"
                  :key="'script-'+index"
                  class="result-item glass-hover interactive"
                  :class="{'result-item-active': selectedIndex === (1 + filteredWindows.length + filteredApps.length + index)}"
                  @click="executeScript(script)"
                >
                  <div class="result-icon text-success">
                      <v-icon icon="mdi-console-line" size="20"></v-icon>
                  </div>
                  <div class="result-content">
                    <div class="result-title" v-html="highlightMatch(script.alias)"></div>
                    <div class="result-subtitle text-dim font-mono text-xs">{{ script.path }}</div>
                  </div>
                </div>
              </div>

              <!-- Files -->
              <div v-if="files.length" class="results-section">
                <div class="section-header">FILES</div>
                <div
                  v-for="(file, index) in files"
                  :key="'file-'+index"
                  class="result-item glass-hover interactive"
                  :class="{'result-item-active': selectedIndex === (1 + filteredWindows.length + filteredApps.length + filteredScripts.length + index)}"
                  @click="executeFile(file)"
                >
                  <div class="result-icon">
                      <v-icon :icon="getFileIcon(file)" :class="getFileColor(file)" size="20"></v-icon>
                  </div>
                  <div class="result-content">
                    <div class="result-title" v-html="highlightMatch(getFileName(file))"></div>
                    <div class="result-subtitle text-dim text-xs truncate">{{ file }}</div>
                  </div>
                </div>
              </div>
            </div>
        </div>

        <!-- Preview Column -->
        <div v-if="isFileSearchMode" class="preview-col">
          <FilePreview v-if="selectedFile" :file-path="selectedFile" />
          <div v-else class="d-flex align-center justify-center h-100 text-dimmer">
              <div class="text-center">
                  <v-icon icon="mdi-file-search-outline" size="64" class="mb-4 opacity-50"></v-icon>
                  <div>Select a file to preview</div>
              </div>
          </div>
        </div>
    </div>

    <!-- Launch error toast (visible whenever a launch error is set) -->
    <div
      v-if="launchError"
      class="footer footer-launch-error"
      data-testid="launch-error"
    >
      <span class="footer-error text-caption text-error">{{ launchError }}</span>
      <button class="footer-btn interactive" @click="showSettings = true">
        <span>⚙️</span>
        <span class="text-dimmer">[Ctrl+,]</span>
      </button>
    </div>

    <!-- Footer with settings -->
    <div v-if="uiState === 'searching' && !launchError" class="footer">
      <button class="footer-btn interactive" @click="showSettings = true">
        <span>⚙️</span>
        <span class="text-dimmer">[Ctrl+,]</span>
      </button>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, ref } from 'vue'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import CurrencyResult from '../CurrencyResult.vue'
import TopAppsRail from '../TopAppsRail.vue'
import CategoryFilterBar from '../CategoryFilterBar.vue'
import { useOmnibar } from '../../composables/useOmnibar'
import { useAI } from '../../composables/useAI'
import { useScriptRunner } from '../../composables/useScriptRunner'

const emit = defineEmits(['close'])

const {
  uiState, query, searchInput, selectedIndex, showSettings,
  matchedTool, filteredWindows, filteredApps, filteredScripts, files,
  focusWindow, hideWindow,
  recentActions, recordAction, clearActions,
  topApps, scoredApps,
  apps,
  kindFilter, KIND_FILTER_OPTIONS,
  setKindFilter, isKindFilterActive,
} = useOmnibar()

const activeCategory = ref('')
const launchError = ref('')

const categoryOptions = computed(() => {
  const counts = new Map()
  const list = Array.isArray(apps.value) ? apps.value : []
  for (const app of list) {
    const cats = Array.isArray(app && app.categories) ? app.categories : []
    for (const raw of cats) {
      if (typeof raw !== 'string') continue
      const key = raw.trim().toLowerCase()
      if (!key) continue
      counts.set(key, (counts.get(key) || 0) + 1)
    }
  }
  return Array.from(counts.entries())
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .slice(0, 8)
    .map(([id, count]) => ({
      id,
      label: id.charAt(0).toUpperCase() + id.slice(1),
      count,
    }))
})

const appsByCategory = computed(() => {
  if (!activeCategory.value) return filteredApps.value
  const target = activeCategory.value.toLowerCase()
  return filteredApps.value.filter((app) => {
    if (!app) return false
    const cats = Array.isArray(app.categories) ? app.categories : []
    return cats.some((c) => typeof c === 'string' && c.toLowerCase() === target)
  })
})

function onCategorySelect(categoryId) {
  activeCategory.value = categoryId || ''
}

async function onTopAppLaunch(app) {
  await executeApp(app)
}

const { askAI, executeAiTool, executeSkill } = useAI()
const { executeScript } = useScriptRunner()

async function handleClear() {
  // Small delay to prevent click propagation issues
  await nextTick()
  if (confirm('Clear recent actions?')) {
     await clearActions()
  }
}

function handleEsc() {
  if (query.value) {
    query.value = ''
  } else {
    emit('close')
  }
}

const KIND_SHORTCUTS = {
  all: 'Ctrl+0',
  apps: 'Ctrl+1',
  scripts: 'Ctrl+2',
  files: 'Ctrl+3',
  recent: 'Ctrl+4',
}

const kindFilterOptions = KIND_FILTER_OPTIONS

function shortcutFor(optionId) {
  return KIND_SHORTCUTS[optionId] || ''
}

function handleKindShortcut(kindId) {
  setKindFilter(kindId)
}


const isFileSearchMode = computed(() => {
  return query.value && query.value.trim().toLowerCase().startsWith('ff ')
})

// Computed totals for navigation calculation
const isDefaultState = computed(() => !query.value)

const totalItems = computed(() => {
  if (isDefaultState.value) {
      return 1 + (recentActions.value ? recentActions.value.length : 0) + (topApps.value ? topApps.value.length : 0)
  }
  return 1 + filteredWindows.value.length + appsByCategory.value.length + filteredScripts.value.length + files.value.length
})

const topSectionHeader = computed(() => {
  if (matchedTool.value) {
    if (matchedTool.value.type === 'script') return 'MATCHED SCRIPT'
    if (matchedTool.value.type === 'app') return 'MATCHED APP'
  }
  return 'AI ACTIONS'
})

function navigateResults(direction) {
  const max = totalItems.value - 1
  if (max < 0) return
  let newIndex = selectedIndex.value + direction
  if (newIndex < 0) newIndex = max
  if (newIndex > max) newIndex = 0
  selectedIndex.value = newIndex
  
  nextTick(() => {
    const activeItem = document.querySelector('.result-item-active')
    if (activeItem) {
      activeItem.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
    }
  })
}

async function executeAction(index) {
  if (isDefaultState.value) {
    if (index === 0) {
      showSettings.value = true
    } else {
        const action = recentActions.value[index - 1]
        if (action) {
            // Re-execute based on kind
            if (action.kind === 'app') {
               await executeApp({ exec: action.content, name: action.name }) // Reconstruct basic app object
            } else if (action.kind === 'script') {
               await executeScript({ path: action.content, alias: action.name })
            } else if (action.kind === 'file') {
               executeFile(action.content)
            }
            // recordAction is called inside these functions now
            // But for safety/refresh we might want to call it here too? 
            // No, the delegates will call it.
        }
    }
    return
  }

  if (index === 0) {
    if (matchedTool.value) {
      if (matchedTool.value.type === 'app') {
        await executeApp(matchedTool.value.data)
      } else if (matchedTool.value.type === 'script') {
        await executeScript(matchedTool.value.data)
        query.value = ''
      } else if (matchedTool.value.type === 'skill') {
        await executeSkill(matchedTool.value)
        query.value = ''
      } else if (matchedTool.value.type === 'internal' && matchedTool.value.id === 'settings') {
        showSettings.value = true
      } else {
        await executeAiTool(matchedTool.value)
      }
    } else {
      askAI()
    }
    return
  }
  
  let currentIndex = 1
  if (index < currentIndex + filteredWindows.value.length) {
    await focusWindow(filteredWindows.value[index - currentIndex])
    return
  }
  currentIndex += filteredWindows.value.length

  if (index < currentIndex + appsByCategory.value.length) {
    await executeApp(appsByCategory.value[index - currentIndex])
    return
  }
  currentIndex += appsByCategory.value.length
  
  if (index < currentIndex + filteredScripts.value.length) {
    await executeScript(filteredScripts.value[index - currentIndex])
    query.value = ''
    return
  }
  currentIndex += filteredScripts.value.length
  
  if (files.value[index - currentIndex]) {
    executeFile(files.value[index - currentIndex])
  }
}

async function executeApp(app) {
  launchError.value = ''
  try {
    await invoke('launch_app', { execCmd: app.exec })
    recordAction(app)
    query.value = ''
    await hideWindow()
  } catch(e) {
    console.error('Failed to launch app', e)
    launchError.value = typeof e === 'string' ? e : (e && e.message) ? e.message : String(e)
  }
}

async function executeFile(path) {
  try {
    await invoke('open_entity', { path })
    recordAction(path)
    query.value = ''
    await hideWindow()
  } catch(e) {
    console.error('Failed to open file', e)
  }
}

function getFileName(path) {
  return path.split('/').pop()
}

// ... imports
import FilePreview from './FilePreview.vue'

// ... existing code ...

const selectedFile = computed(() => {
  if (!isFileSearchMode.value) return null
  
  // Calculate offset to find if we are on a file
  // Order: Windows -> Apps -> Scripts -> Files
  // Check executeAction logic for offsets
  
  let offset = 1; // Settings/AI
  offset += filteredWindows.value.length
  offset += filteredApps.value.length
  offset += filteredScripts.value.length
  
  const fileIndex = selectedIndex.value - offset
  if (fileIndex >= 0 && fileIndex < files.value.length) {
    return files.value[fileIndex]
  }
  return null
})

function highlightMatch(text) {
  if (!query.value) return text
  // Remove "ff " if in file mode
  let q = query.value
  if (isFileSearchMode.value) q = q.substring(3).trim()
  if (!q) return text

  const regex = new RegExp(`(${q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, 'gi')
  return text.replace(regex, '<span class="text-gradient">$1</span>')
}

function sourceLabel(source) {
  if (!source) return ''
  const normalized = String(source).toLowerCase()
  return normalized.charAt(0).toUpperCase() + normalized.slice(1)
}

function appScoreFor(app) {
  if (!app || !scoredApps.value) return null
  const match = scoredApps.value.find((s) => s && s.app && s.app.id === app.id)
  return match ? match.score : null
}

function formatScore(score) {
  if (score === null || score === undefined) return ''
  if (typeof score !== 'number') return ''
  return score.toFixed(2)
}

function getFileIcon(path) {
    const ext = path.split('.').pop().toLowerCase()
    if (['png','jpg','jpeg','webp','gif','svg'].includes(ext)) return 'mdi-image'
    if (['mp4','mkv','avi','mov','webm'].includes(ext)) return 'mdi-movie'
    if (['mp3','wav','ogg'].includes(ext)) return 'mdi-music'
    if (['js','ts','vue','py','rs','html','css','json','c','cpp'].includes(ext)) return 'mdi-code-braces'
    if (ext === 'pdf') return 'mdi-file-pdf-box'
    if (ext === 'md') return 'mdi-language-markdown'
    return 'mdi-file'
}

function getFileColor(path) {
    const ext = path.split('.').pop().toLowerCase()
    if (['png','jpg','jpeg','webp','gif','svg'].includes(ext)) return 'text-purple-300'
    if (['mp4','mkv','avi','mov','webm'].includes(ext)) return 'text-red-300'
    if (['js','ts','vue','py','rs'].includes(ext)) return 'text-yellow-300'
    return 'text-blue-300'
}

</script>

<style scoped>
/* Copied from App.vue and Adapted */
.omnibar-search-mode {
  width: 100%;
  max-width: 100%;
  height: 100%;
  background: var(--theme-background);
  box-shadow: var(--shadow-xl);
  border: 1px solid var(--theme-border);
  overflow: hidden;
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

.search-mode-badge {
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

/* MAIN LAYOUT */
.main-content {
    flex: 1;
    min-height: 0;
    display: flex;
    overflow: hidden;
}

.results-col {
  flex: 1;
  overflow-y: auto;
  padding: 0 var(--space-4) var(--space-4);
  min-width: 0; 
}

/* Dual Pane Overlay */
.preview-col {
  width: 60%;
  border-left: 1px solid var(--theme-border);
  background: rgba(0, 0, 0, 0.2);
  display: flex;
  flex-direction: column;
}

/* File Search Improvements */
.results-section {
  margin-bottom: var(--space-4);
}

.section-header {
  font-size: var(--font-size-xs);
  font-weight: var(--font-weight-semibold);
  color: var(--theme-text-dimmer);
  letter-spacing: 0.05em;
  padding: var(--space-2) var(--space-4);
  margin-bottom: var(--space-2);
}

.result-item {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
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
  /* Glow effect */
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

.result-hint {
  font-size: var(--font-size-xs);
  font-family: var(--font-mono);
  flex-shrink: 0;
}

.footer {
  flex-shrink: 0;
  padding: var(--space-2) var(--space-4);
  border-top: 1px solid var(--theme-border);
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--space-3);
}

.footer-error {
  margin-right: auto;
  font-weight: var(--font-weight-medium);
  opacity: 0.9;
}

.footer-btn {
  background: transparent;
  border: none;
  color: var(--theme-text-dim);
  font-size: var(--font-size-sm);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.truncate {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.ml-2 { margin-left: var(--space-2); }
.text-xs { font-size: var(--font-size-xs); }
.text-purple-300 { color: #d8b4fe; }
.text-red-300 { color: #fca5a5; }
.text-yellow-300 { color: #fde047; }
.text-blue-300 { color: #93c5fd; }

.clear-btn {
  font-size: 10px;
  color: var(--theme-text-dimmer);
  opacity: 0.6;
  border: none;
  background: transparent;
  cursor: pointer;
  padding: 2px 6px;
  border-radius: 4px;
  transition: all 0.2s;
}

.clear-btn:hover {
  opacity: 1;
  background: rgba(255, 50, 50, 0.1);
  color: #fca5a5;
}

.source-badge {
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  padding: 2px 6px;
  border-radius: 4px;
  margin-left: auto;
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

.score-hint {
  font-size: 10px;
  font-family: var(--font-mono, monospace);
  flex-shrink: 0;
  margin-left: 6px;
  padding: 2px 5px;
  border-radius: 3px;
  background: rgba(255, 255, 255, 0.04);
}

.top-app-item {
  border-left: 2px solid rgba(122, 162, 247, 0.2);
}

.kind-filter-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: 0 var(--space-6) var(--space-2);
  flex-shrink: 0;
  overflow-x: auto;
  scrollbar-width: none;
}

.kind-filter-row::-webkit-scrollbar {
  display: none;
}

.kind-chip {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: 4px var(--space-3);
  border-radius: 999px;
  border: 1px solid var(--theme-border);
  background: rgba(255, 255, 255, 0.03);
  color: var(--theme-text-dim);
  font-size: var(--font-size-xs);
  font-weight: var(--font-weight-semibold);
  letter-spacing: 0.04em;
  cursor: pointer;
  white-space: nowrap;
  transition: all var(--duration-fast) var(--ease-out);
}

.kind-chip:hover {
  background: rgba(255, 255, 255, 0.06);
  color: var(--theme-text);
}

.kind-chip-active {
  background: rgba(122, 162, 247, 0.2);
  border-color: rgba(122, 162, 247, 0.5);
  color: var(--theme-primary);
  box-shadow: inset 0 0 0 1px rgba(122, 162, 247, 0.2);
}

.kind-chip-label {
  font-size: var(--font-size-xs);
}

.kind-chip-hint {
  font-size: 10px;
  font-family: var(--font-mono, monospace);
  opacity: 0.7;
}
</style>
