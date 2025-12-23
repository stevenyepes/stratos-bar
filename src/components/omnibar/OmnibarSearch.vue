<template>
  <div class="omnibar-search-mode scale-in">
    <!-- Drag handle -->
    <div data-tauri-drag-region class="drag-handle"></div>
    
    <!-- Search input -->
    <div class="search-container">
      <input
        ref="searchInput"
        v-model="query"
        type="text"
        :placeholder="uiState === 'idle' ? '🔍 Type a command, search files, or ask AI...' : 'Type to search...'"
        class="search-input font-primary"
        autofocus
        @keydown.down.prevent="navigateResults(1)"
        @keydown.up.prevent="navigateResults(-1)"
        @keydown.enter.prevent="executeAction(selectedIndex)"
        @keydown.esc="emit('close')"
        @keydown.ctrl.n.prevent="askAI"
      />
    </div>

    <!-- Empty state (Default Items) -->
    <div v-if="!query" class="results-container custom-scrollbar fade-in">
       <div class="results-section">
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
    </div>

    <!-- Results -->
    <div v-else class="results-container custom-scrollbar fade-in">
      
      <!-- AI Actions Section -->
      <div v-if="matchedTool || query" class="results-section">
        <div class="section-header">{{ topSectionHeader }}</div>
        
        <!-- Special Result: Currency -->
        <CurrencyResult 
          v-if="matchedTool && matchedTool.data && matchedTool.data.type === 'currency'"
          :data="matchedTool.data"
          @execute="executeAction(0)"
          @swap="swapCurrencyQuery(matchedTool.data)"
        />

        <!-- Matched AI Tool/Skill (Standard) -->
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

        <!-- General AI  -->
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
            <div class="result-title">{{ win.title }}</div>
            <div class="result-subtitle text-dim">Switch to {{ win.class }}</div>
          </div>
        </div>
      </div>

      <!-- System Commands / Apps -->
      <div v-if="filteredApps.length" class="results-section">
        <div class="section-header">APPLICATIONS</div>
        <div
          v-for="(app, index) in filteredApps"
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
            <div class="result-title">{{ app.name }}</div>
            <div class="result-subtitle text-dim">{{ app.exec }}</div>
          </div>
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
            <div class="result-title">{{ script.alias }}</div>
            <div class="result-subtitle text-dim font-mono text-xs">{{ script.path }} {{script.args || ''}}</div>
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
          <div class="result-icon">📄</div>
          <div class="result-content">
            <div class="result-title">{{ getFileName(file) }}</div>
            <div class="result-subtitle text-dim text-xs truncate">{{ file }}</div>
          </div>
        </div>
      </div>
    </div>

    <!-- Footer with settings -->
    <div v-if="uiState === 'searching'" class="footer">
      <button class="footer-btn interactive" @click="showSettings = true">
        <span>⚙️</span>
        <span class="text-dimmer">[Ctrl+,]</span>
      </button>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick } from 'vue'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import CurrencyResult from '../CurrencyResult.vue'
import { useOmnibar } from '../../composables/useOmnibar'
import { useAI } from '../../composables/useAI'
import { useScriptRunner } from '../../composables/useScriptRunner'

const emit = defineEmits(['close'])

const { 
  uiState, query, searchInput, selectedIndex, showSettings,
  matchedTool, filteredWindows, filteredApps, filteredScripts, files,
  focusWindow, hideWindow
} = useOmnibar()

const { askAI, executeAiTool, executeSkill } = useAI()
const { executeScript } = useScriptRunner()

// Computed totals for navigation calculation
const isDefaultState = computed(() => !query.value)

const totalItems = computed(() => {
  if (isDefaultState.value) return 1
  return 1 + filteredWindows.value.length + filteredApps.value.length + filteredScripts.value.length + files.value.length
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
    }
    return
  }

  if (index === 0) {
    if (matchedTool.value) {
      if (matchedTool.value.type === 'app') {
        await executeApp(matchedTool.value.data)
      } else if (matchedTool.value.type === 'script') {
        await executeScript(matchedTool.value.data)
      } else if (matchedTool.value.type === 'skill') {
        await executeSkill(matchedTool.value)
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

  if (index < currentIndex + filteredApps.value.length) {
    await executeApp(filteredApps.value[index - currentIndex])
    return
  }
  currentIndex += filteredApps.value.length
  
  if (index < currentIndex + filteredScripts.value.length) {
    await executeScript(filteredScripts.value[index - currentIndex])
    return
  }
  currentIndex += filteredScripts.value.length
  
  if (files.value[index - currentIndex]) {
    executeFile(files.value[index - currentIndex])
  }
}

async function executeApp(app) {
  try {
    await invoke('launch_app', { execCmd: app.exec })
    await hideWindow()
  } catch(e) {
    console.error('Failed to launch app', e)
  }
}

async function executeFile(path) {
  try {
    await invoke('open_entity', { path })
    await hideWindow()
  } catch(e) {
    console.error('Failed to open file', e)
  }
}

function getFileName(path) {
  return path.split('/').pop()
}

function swapCurrencyQuery(data) {
  if (!data) return
  query.value = `${data.amount} ${data.to} to ${data.from}`
  
  nextTick(() => {
    if (searchInput.value) searchInput.value.focus()
  })
}
</script>

<style scoped>
/* Copied from App.vue */
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

.results-container {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 0 var(--space-4) var(--space-4);
}

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
}

.result-item:hover {
  background: rgba(255, 255, 255, 0.05);
}

.result-item-active {
  background: rgba(122, 162, 247, 0.25) !important;
  border: 1px solid rgba(122, 162, 247, 0.5);
  box-shadow: 0 0 0 1px rgba(122, 162, 247, 0.3);
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
  justify-content: flex-end;
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
</style>
