<template>
  <v-app theme="dark" style="height: 100vh; background: transparent;">
    <v-main class="pa-0" style="height: 100vh; background: transparent;">
      <div class="omnibar-container" :class="{'omnibar-expanded': uiState !== 'idle'}">
        
        <!-- State 1: Idle / State 2: Searching -->
        <div v-if="uiState !== 'chatting'" class="omnibar-search-mode scale-in">
          
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
              @keydown.esc="handleEsc"
            />
          </div>

          <!-- State 1: Empty state -->
          <div v-if="!query" class="empty-state">
            <p class="text-dimmer">No recent items</p>
            <p class="text-dimmer text-xs mt-2">[esc] to close</p>
          </div>

          <!-- State 2: Results -->
          <div v-else class="results-container custom-scrollbar fade-in">
            
            <!-- AI Actions Section -->
            <div v-if="matchedTool || query" class="results-section">
              <div class="section-header">AI ACTIONS</div>
              
              <!-- Matched AI Tool/Skill -->
              <div 
                v-if="matchedTool"
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
                <div class="result-hint text-dimmer">[↵]</div>
              </div>
            </div>

            <!-- System Commands / Apps -->
            <div v-if="filteredApps.length" class="results-section">
              <div class="section-header">APPLICATIONS</div>
              <div
                v-for="(app, index) in filteredApps"
                :key="'app-'+index"
                class="result-item glass-hover interactive"
                :class="{'result-item-active': selectedIndex === (1 + index)}"
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
                :class="{'result-item-active': selectedIndex === (1 + filteredApps.length + index)}"
                @click="executeScript(script)"
              >
                <div class="result-icon">📜</div>
                <div class="result-content">
                  <div class="result-title">{{ getFileName(script) }}</div>
                  <div class="result-subtitle text-dim font-mono">{{ script }}</div>
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
                :class="{'result-item-active': selectedIndex === (1 + filteredApps.length + filteredScripts.length + index)}"
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
              <span class="text-dimmer">[⇧⌘P]</span>
            </button>
          </div>
        </div>

        <!-- State 3: Chat Mode -->
        <div v-else class="omnibar-chat-mode scale-in">
          
          <!-- Chat Header -->
          <div data-tauri-drag-region class="chat-header">
            <button class="back-btn interactive" @click="closeAiChat">
              <span>←</span>
              <span class="ml-2">Back</span>
            </button>
            <div class="flex-grow"></div>
            <button class="menu-btn interactive" @click="showSettings = true">
              <span>⋯</span>
            </button>
          </div>

          <!-- Chat Messages -->
          <div ref="messagesContainer" class="chat-messages custom-scrollbar">
            <div v-if="chatMessages.length === 0" class="empty-chat">
              <p class="text-dim">Start a conversation...</p>
            </div>
            
            <div v-for="(msg, i) in chatMessages" :key="i" class="message-wrapper fade-in">
              <!-- User Message -->
              <div v-if="msg.role === 'user'" class="message message-user">
                <div class="message-avatar">👤</div>
                <div>
                  <div class="message-label text-dim">You</div>
                  <div class="message-text">{{ msg.content }}</div>
                </div>
              </div>

              <!-- AI Message -->
              <div v-else class="message message-ai">
                <div class="message-avatar">🤖</div>
                <div class="message-ai-content">
                  <div class="message-label text-dim">AI</div>
                  <div v-html="renderMarkdown(msg.content)" class="message-text markdown-body"></div>
                  
                  <!-- Micro-interactions (hover-revealed) -->
                  <div class="message-actions">
                    <button class="action-btn interactive" @click="copyMessage(msg.content)" title="Copy">
                      📄
                    </button>
                    <button class="action-btn interactive" title="Regenerate">
                      🔄
                    </button>
                  </div>
                </div>
              </div>
            </div>

            <div v-if="chatLoading" class="message message-ai">
              <div class="message-avatar">🤖</div>
              <div>
                <div class="message-label text-dim">AI</div>
                <div class="typing-indicator">
                  <span></span><span></span><span></span>
                </div>
              </div>
            </div>
          </div>

          <!-- Chat Input -->
          <div class="chat-input-container">
            <input
              v-model="chatInput"
              type="text"
              placeholder="Reply to continue conversation..."
              class="chat-input font-primary"
              @keydown.enter.prevent="sendChatMessage"
            />
            <button 
              class="send-btn interactive"
              :disabled="!chatInput.trim()"
              @click="sendChatMessage"
            >
              <span>➤</span>
            </button>
          </div>
        </div>

        <!-- Settings Component (Overlay) -->
        <Settings 
          v-model="showSettings" 
          :initial-config="config"
          :apps="apps"
          @config-updated="handleConfigUpgrade"
        />

      </div>
    </v-main>
  </v-app>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { getCurrentWindow, currentMonitor } from '@tauri-apps/api/window'
import { LogicalSize } from '@tauri-apps/api/dpi'
import Settings from './components/Settings.vue'
import SkillManager from './skills'
import { applyTheme } from './theme'
import { useTheme } from 'vuetify'
import { marked } from 'marked'
import hljs from 'highlight.js'
import 'highlight.js/styles/atom-one-dark.css'

const vTheme = useTheme()

// UI State Management
const uiState = ref('idle') // 'idle', 'searching', 'chatting'
const query = ref('')
const chatInput = ref('')
const chatMessages = ref([])
const chatLoading = ref(false)

// Data
const apps = ref([])
const files = ref([])
const scripts = ref([])
const config = ref(null)
const selectedIndex = ref(0)
const showSettings = ref(false)
const searchInput = ref(null)
const messagesContainer = ref(null)

const appWindow = getCurrentWindow()

// Configure marked
const renderer = new marked.Renderer()
renderer.code = ({ text, lang }) => {
  const validLang = !!(lang && hljs.getLanguage(lang))
  const highlighted = validLang ? hljs.highlight(text, { language: lang }).value : text
  const langLabel = lang ? lang : 'text'
  
  return `
    <div class="code-block">
      <div class="code-header">
        <span class="code-lang font-mono">${langLabel}</span>
        <button class="code-copy-btn" data-code="${text.replace(/"/g, '&quot;')}">Copy</button>
      </div>
      <pre><code class="hljs ${lang}">${highlighted}</code></pre>
    </div>
  `
}
marked.use({ renderer })

// Load initial data
onMounted(async () => {
  try {
    apps.value = await invoke('list_apps')
    scripts.value = await invoke('list_scripts')
    await reloadConfig()
    await updateWindowSize()
  } catch (e) {
    console.error('Failed to load initial data', e)
  }
  window.addEventListener('keydown', handleGlobalKeydown)
})

onUnmounted(() => {
  window.removeEventListener('keydown', handleGlobalKeydown)
})

function handleGlobalKeydown(e) {
  if (e.key === 'Escape') {
    handleEsc()
  }
}

async function reloadConfig() {
  config.value = await invoke('get_config')
  if (!config.value.shortcuts) config.value.shortcuts = {}
  
  if (config.value.theme) {
    applyTheme(config.value.theme)
    vTheme.themes.value.dark.colors.primary = config.value.theme.primary
    vTheme.themes.value.dark.colors.secondary = config.value.theme.secondary
  }
}

function handleConfigUpgrade(newConfig) {
  config.value = newConfig
}

// Watch query to update UI state
watch(query, (newVal) => {
  selectedIndex.value = 0
  if (newVal && uiState.value === 'idle') {
    uiState.value = 'searching'
    updateWindowSize()
  } else if (!newVal && uiState.value === 'searching') {
    uiState.value = 'idle'
    updateWindowSize()
  }
  
  // File search
  if (!newVal) {
    files.value = []
  } else {
    clearTimeout(window.searchTimeout)
    window.searchTimeout = setTimeout(async () => {
      try {
        files.value = await invoke('search_files', { query: newVal, path: '/home/jsteven' })
      } catch (e) {
        console.error(e)
      }
    }, 300)
  }
})

// Window sizing based on state
const COLLAPSED_HEIGHT = 100
const EXPANDED_HEIGHT = 500
const CHAT_HEIGHT = 600
const BASE_WIDTH = 700

async function updateWindowSize() {
  try {
    const monitor = await currentMonitor()
    let width = BASE_WIDTH
    let height = COLLAPSED_HEIGHT
    
    if (monitor) {
      const scaleFactor = monitor.scaleFactor
      const screenWidth = monitor.size.width / scaleFactor
      width = Math.max(BASE_WIDTH, Math.floor(screenWidth * 0.4))
    }
    
    if (uiState.value === 'chatting') {
      height = CHAT_HEIGHT
    } else if (uiState.value === 'searching') {
      height = EXPANDED_HEIGHT
    }
    
    await appWindow.setSize(new LogicalSize(width, height))
  } catch (e) {
    console.error('Failed to resize window:', e)
  }
}

// Tool matching logic
const matchedTool = computed(() => {
  if (!query.value) return null
  const q = query.value.toLowerCase()
  const tools = (config.value && config.value.ai_tools) || []
  const shortcuts = (config.value && config.value.shortcuts) || {}
  
  // Check shortcuts
  if (shortcuts[q]) {
    const targetId = shortcuts[q]
    if (targetId.startsWith('app:')) {
      const exec = targetId.substring(4)
      const app = apps.value.find(a => a.exec === exec)
      if (app) {
        return {
          type: 'app',
          name: app.name,
          description: 'Launch application',
          icon: '🚀',
          data: app
        }
      }
    }
    const tool = tools.find(t => t.id === targetId)
    if (tool) return { type: 'tool', ...tool }
  }
  
  // Check keywords
  for (const tool of tools) {
    if (tool.keywords && tool.keywords.some(k => q.startsWith(k.toLowerCase()))) {
      return { type: 'tool', ...tool }
    }
  }
  
  // Check SkillManager
  const skillMatch = SkillManager.match(q)
  if (skillMatch) {
    return {
      type: 'skill',
      name: skillMatch.skill.name,
      description: skillMatch.preview || skillMatch.skill.description,
      icon: skillMatch.skill.icon,
      skill: skillMatch.skill,
      data: skillMatch.data
    }
  }
  
  return null
})

const filteredApps = computed(() => {
  if (!query.value) return []
  return apps.value.filter(app => 
    app.name.toLowerCase().includes(query.value.toLowerCase()) ||
    app.exec.toLowerCase().includes(query.value.toLowerCase())
  ).slice(0, 5)
})

const filteredScripts = computed(() => {
  if (!query.value) return scripts.value
  return scripts.value.filter(s => s.toLowerCase().includes(query.value.toLowerCase()))
})

const totalItems = computed(() => 1 + filteredApps.value.length + filteredScripts.value.length + files.value.length)

function navigateResults(direction) {
  const max = totalItems.value - 1
  if (max < 0) return
  let newIndex = selectedIndex.value + direction
  if (newIndex < 0) newIndex = max
  if (newIndex > max) newIndex = 0
  selectedIndex.value = newIndex
  
  // Scroll the selected item into view
  nextTick(() => {
    const activeItem = document.querySelector('.result-item-active')
    if (activeItem) {
      activeItem.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
    }
  })
}

async function executeAction(index) {
  if (index === 0) {
    if (matchedTool.value) {
      if (matchedTool.value.type === 'app') {
        await executeApp(matchedTool.value.data)
      } else if (matchedTool.value.type === 'skill') {
        await executeSkill(matchedTool.value)
      } else {
        executeAiTool(matchedTool.value)
      }
    } else {
      askAI()
    }
    return
  }
  
  let currentIndex = 1
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

async function executeAiTool(tool) {
  try {
    let prompt = tool.prompt_template
    if (prompt.includes('{{selection}}')) {
      const text = await invoke('get_selection_context')
      prompt = prompt.replace('{{selection}}', text)
    }
    
    chatMessages.value.push({ role: 'user', content: prompt })
    uiState.value = 'chatting'
    updateWindowSize()
    await sendChatMessage(null, true)
  } catch(e) {
    console.error('Failed to execute tool', e)
  }
}

async function executeSkill(match) {
  try {
    const result = await match.skill.execute(match.data)
    if (result !== undefined && result !== null) {
      try {
        await invoke('copy_to_clipboard', { text: result.toString() })
        await new Promise(resolve => setTimeout(resolve, 200))
      } catch (e) {
        console.error('Clipboard failed', e)
      }
      await hideWindow()
    }
  } catch(e) {
    console.error('Failed to execute skill', e)
  }
}

async function executeScript(path) {
  try {
    await invoke('launch_app', { execCmd: path }) 
    await hideWindow()
  } catch(e) {
    console.error(e)
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

function askAI() {
  if (!query.value) return
  chatMessages.value.push({ role: 'user', content: query.value })
  uiState.value = 'chatting'
  updateWindowSize()
  sendChatMessage(null, true)
}

async function sendChatMessage(e, skipUserAdd = false) {
  if ((!chatInput.value.trim() && !skipUserAdd) || chatLoading.value) return
  
  if (!skipUserAdd) {
    chatMessages.value.push({ role: 'user', content: chatInput.value })
    chatInput.value = ''
  }
  
  chatLoading.value = true
  scrollToBottom()
  
  try {
    const history = JSON.parse(JSON.stringify(chatMessages.value))
    const response = await invoke('ask_ai', { messages: history })
    chatMessages.value.push({ role: 'assistant', content: response })
  } catch(err) {
    console.error(err)
    chatMessages.value.push({ role: 'assistant', content: 'Error: ' + err })
  } finally {
    chatLoading.value = false
    scrollToBottom()
  }
}

function scrollToBottom() {
  nextTick(() => {
    if (messagesContainer.value) {
      messagesContainer.value.scrollTop = messagesContainer.value.scrollHeight
    }
  })
}

function renderMarkdown(text) {
  try {
    return marked.parse(text)
  } catch (e) {
    return text
  }
}

async function copyMessage(content) {
  try {
    await navigator.clipboard.writeText(content)
  } catch(e) {
    console.error('Failed to copy', e)
  }
}

function closeAiChat() {
  uiState.value = query.value ? 'searching' : 'idle'
  chatMessages.value = []
  chatInput.value = ''
  updateWindowSize()
}

function handleEsc() {
  if (showSettings.value) {
    showSettings.value = false
    return
  }
  if (uiState.value === 'chatting') {
    closeAiChat()
  } else {
    hideWindow()
  }
}

async function hideWindow() {
  query.value = ''
  uiState.value = 'idle'
  chatMessages.value = []
  await appWindow.hide()
}

function getFileName(path) {
  return path.split('/').pop()
}
</script>

<style scoped>
/* Omnibar Container */
.omnibar-container {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding: 0;
  transition: all var(--duration-slow) var(--ease-in-out);
}

.omnibar-expanded {
  align-items: flex-start;
}

/* Search Mode */
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

.empty-state {
  padding: var(--space-8) var(--space-6);
  text-align: center;
  flex-shrink: 0;
}

.results-container {
  max-height: 380px;
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

/* Chat Mode */
.omnibar-chat-mode {
  width: 100%;
  height: 100%;
  background: var(--theme-background);
  box-shadow: var(--shadow-xl);
  border: 1px solid var(--theme-border);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.chat-header {
  padding: var(--space-4) var(--space-6);
  border-bottom: 1px solid var(--theme-border);
  display: flex;
  align-items: center;
  gap: var(--space-3);
  flex-shrink: 0;
}

.back-btn, .menu-btn {
  background: transparent;
  border: none;
  color: var(--theme-text);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  font-size: var(--font-size-sm);
  display: flex;
  align-items: center;
}

.chat-messages {
  flex: 1;
  overflow-y: auto;
  padding: var(--space-6);
}

.message-text {
  color: var(--theme-text);
  font-size: var(--font-size-base);
  line-height: var(--line-height-relaxed);
}

.message-text p {
  margin-bottom: 0.5em;
}

.message-text p:last-child {
  margin-bottom: 0;
}

.message-text pre {
  margin: 0.5em 0;
  padding: 0.5em;
  border-radius: var(--radius-sm);
  background: rgba(0, 0, 0, 0.2);
  overflow-x: auto;
}

.empty-chat {
  text-align: center;
  padding: var(--space-10) 0;
}

.message-wrapper {
  margin-bottom: var(--space-6);
}

.message {
  display: flex;
  gap: var(--space-3);
  align-items: flex-start;
}

.message-avatar {
  font-size: 24px;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.message-label {
  font-size: var(--font-size-xs);
  font-weight: var(--font-weight-semibold);
  margin-bottom: var(--space-1);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.message-text {
  font-size: var(--font-size-sm);
  line-height: var(--line-height-relaxed);
  color: var(--theme-text);
}

.message-user .message-text {
  opacity: 0.8;
}

.message-ai-content {
  flex: 1;
  position: relative;
}

.message-actions {
  display: none;
  gap: var(--space-2);
  margin-top: var(--space-2);
}

.message-ai-content:hover .message-actions {
  display: flex;
}

.action-btn {
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid var(--theme-border);
  border-radius: var(--radius-md);
  padding: var(--space-2);
  font-size: 16px;
  cursor: pointer;
}

.typing-indicator {
  display: flex;
  gap: 4px;
  padding: var(--space-2) 0;
}

.typing-indicator span {
  width: 6px;
  height: 6px;
  background: var(--theme-text-dim);
  border-radius: 50%;
  animation: typing 1.4s infinite;
}

.typing-indicator span:nth-child(2) {
  animation-delay: 0.2s;
}

.typing-indicator span:nth-child(3) {
  animation-delay: 0.4s;
}

@keyframes typing {
  0%, 60%, 100% { opacity: 0.3; transform: translateY(0); }
  30% { opacity: 1; transform: translateY(-4px); }
}

.chat-input-container {
  padding: var(--space-4) var(--space-6);
  border-top: 1px solid var(--theme-border);
  display: flex;
  gap: var(--space-3);
  align-items: center;
  flex-shrink: 0;
}

.chat-input {
  flex: 1;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid var(--theme-border);
  border-radius: var(--radius-lg);
  padding: var(--space-3) var(--space-4);
  color: var(--theme-text);
  font-size: var(--font-size-sm);
  outline: none;
}

.chat-input::placeholder {
  color: var(--theme-text-dimmer);
}

.send-btn {
  background: var(--theme-primary);
  border: none;
  border-radius: var(--radius-lg);
  padding: var(--space-3) var(--space-5);
  color: white;
  font-size: 18px;
  cursor: pointer;
  transition: all var(--duration-fast);
}

.send-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.send-btn:not(:disabled):hover {
  transform: scale(1.05);
  box-shadow: var(--shadow-glow);
}

/* Utilities */
.flex-grow {
  flex-grow: 1;
}

.ml-2 {
  margin-left: var(--space-2);
}

.mt-2 {
  margin-top: var(--space-2);
}

.text-xs {
  font-size: var(--font-size-xs);
}

.truncate {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>

<style>
/* Markdown Styles (unscoped) */
.markdown-body {
  font-size: var(--font-size-sm);
  line-height: var(--line-height-relaxed);
}

.markdown-body p {
  margin-bottom: var(--space-3);
}

.markdown-body code {
  background: rgba(0, 0, 0, 0.3);
  padding: 2px 6px;
  border-radius: var(--radius-sm);
  font-family: var(--font-mono);
  font-size: 0.9em;
}

.markdown-body .code-block {
  margin: var(--space-4) 0;
  border-radius: var(--radius-lg);
  overflow: hidden;
  background: rgba(0, 0, 0, 0.4);
  border: 1px solid var(--theme-border);
}

.markdown-body .code-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--space-2) var(--space-4);
  background: rgba(0, 0, 0, 0.3);
  border-bottom: 1px solid var(--theme-border);
}

.markdown-body .code-lang {
  font-size: var(--font-size-xs);
  color: var(--theme-text-dim);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.markdown-body .code-copy-btn {
  background: transparent;
  border: 1px solid var(--theme-border);
  color: var(--theme-primary);
  padding: var(--space-1) var(--space-3);
  border-radius: var(--radius-sm);
  font-size: var(--font-size-xs);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.markdown-body .code-copy-btn:hover {
  background: rgba(122, 162, 247, 0.1);
}

.markdown-body pre {
  margin: 0;
  padding: var(--space-4);
  overflow-x: auto;
}

.markdown-body pre code {
  background: transparent;
  padding: 0;
}
</style>
