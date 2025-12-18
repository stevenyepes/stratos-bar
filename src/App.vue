<template>
  <v-app theme="dark" style="height: 100vh; background: transparent;">
    <v-main class="pa-2" style="height: 100vh; background: transparent;">
      <div class="d-flex fill-height">
      
      <!-- Main Palette Card -->
      <v-card
        class="rounded-xl overflow-hidden glass-effect flex-grow-1 d-flex flex-column"
        elevation="0"
        height="100%"
      >
        <!-- Drag Handle -->
        <div data-tauri-drag-region style="height: 24px; width: 100%; position: absolute; top: 0; left: 0; z-index: 10;" class="cursor-move"></div>

        <!-- Search Input -->
        <div class="px-6 pt-6 pb-2" style="position: relative; z-index: 20;">
          <v-text-field
            v-model="query"
            placeholder="Type a command..."
            variant="solo"
            prepend-inner-icon="mdi-magnify"
            rounded="lg"
            bg-color="grey-darken-3" 
            hide-details
            class="search-bar"
            autofocus
            @keydown.down.prevent="navigateResults(1)"
            @keydown.up.prevent="navigateResults(-1)"
            @keydown.enter.prevent="executeAction(selectedIndex)"
            @keydown.esc="handleEsc"
          ></v-text-field>
        </div>

        <!-- Results List -->
        <v-list class="bg-transparent px-4 py-0 overflow-y-auto flex-grow-1">
          
        <!-- AI Tool Action (Dynamic) -->
        <v-list-item
          v-if="matchedTool"
          :active="selectedIndex === 0"
          rounded="lg"
          class="mb-2 bg-surface-light"
          @click="executeAction(0)"
        >
          <template v-slot:prepend>
            <v-icon :icon="matchedTool.icon || 'mdi-robot'" color="purple-accent-2" class="mr-3"></v-icon>
          </template>
          <v-list-item-title>{{ matchedTool.name }}</v-list-item-title>
          <v-list-item-subtitle>{{ matchedTool.description }}</v-list-item-subtitle>
        </v-list-item>

        <!-- General AI Action (Fallback) -->
        <v-list-subheader v-if="query && !matchedTool">AI Assistant</v-list-subheader>
        <v-list-item
          v-if="query && !matchedTool"
          :active="selectedIndex === 0"
          rounded="lg"
          class="mb-2 bg-surface-light"
          @click="askAI()"
        >
          <template v-slot:prepend>
            <v-icon icon="mdi-robot" color="primary" class="mr-3"></v-icon>
          </template>
          <v-list-item-title>Ask AI: "{{ query }}"</v-list-item-title>
          <v-list-item-subtitle>Get instant answers or text processing</v-list-item-subtitle>
        </v-list-item>

        <!-- Apps -->
        <v-list-subheader v-if="filteredApps.length">Apps</v-list-subheader>
        <v-list-item
          v-for="(app, index) in filteredApps"
          :key="'app-'+index"
          :value="app"
          :active="selectedIndex === (1 + index)"
          rounded="lg"
          class="mb-1"
          @click="executeApp(app)"
        >
          <template v-slot:prepend>
            <v-avatar rounded="0" class="mr-3" v-if="app.icon">
                <v-img :src="convertFileSrc(app.icon)" width="32" height="32"></v-img>
            </v-avatar>
            <v-icon v-else icon="mdi-application" class="mr-3"></v-icon> 
          </template>
          <v-list-item-title>{{ app.name }}</v-list-item-title>
          <v-list-item-subtitle class="text-caption">{{ app.exec }}</v-list-item-subtitle>
        </v-list-item>

        <!-- Scripts -->
        <v-list-subheader v-if="filteredScripts.length">Scripts</v-list-subheader>
         <v-list-item
          v-for="(script, index) in filteredScripts"
          :key="'script-'+index"
          :value="script"
          :active="selectedIndex === (1 + filteredApps.length + index)"
          rounded="lg"
          class="mb-1"
          @click="executeScript(script)"
        >
          <template v-slot:prepend>
            <v-icon icon="mdi-script-text-outline" class="mr-3" color="secondary"></v-icon> 
          </template>
          <v-list-item-title>{{ getFileName(script) }}</v-list-item-title>
          <v-list-item-subtitle class="text-caption">{{ script }}</v-list-item-subtitle>
        </v-list-item>

        <!-- Files -->
        <v-list-subheader v-if="files.length">Files</v-list-subheader>
        <v-list-item
          v-for="(file, index) in files"
          :key="'file-'+index"
          :value="file"
          :active="selectedIndex === (1 + filteredApps.length + filteredScripts.length + index)"

          rounded="lg"
          class="mb-1"
          @click="executeFile(file)"
        >
          <template v-slot:prepend>
            <v-icon icon="mdi-file-outline" class="mr-3"></v-icon>
          </template>
          <v-list-item-title>{{ getFileName(file) }}</v-list-item-title>
          <v-list-item-subtitle class="text-caption text-truncate">{{ file }}</v-list-item-subtitle>
        </v-list-item>

      </v-list>
      
      <!-- Footer / Settings Trigger -->
      <div class="px-4 pb-2 text-right">
        <v-btn icon="mdi-cog" variant="text" size="small" @click="showSettings = true; console.log('Settings button clicked, showSettings:', showSettings)"></v-btn>
      </div>

      <!-- Settings Component -->
      <Settings 
        v-model="showSettings" 
        :initial-config="config"
        :apps="apps"
        @config-updated="handleConfigUpgrade"
      />
      
      </v-card>

      <!-- AI Side Card -->
      <div v-if="showAiChat" class="ml-2 fill-height" style="width: 400px; min-width: 400px;">
         <AiChat 
            :initial-query="initialAiQuery" 
            @close="closeAiChat"
         />
      </div>
      
      </div>
    </v-main>
  </v-app>
</template>

<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { getCurrentWindow, currentMonitor } from '@tauri-apps/api/window'
import { LogicalSize } from '@tauri-apps/api/dpi'
import AiChat from './components/AiChat.vue'
import Settings from './components/Settings.vue'
import SkillManager from './skills'
import { applyTheme } from './theme'
import { useTheme } from 'vuetify'

const vTheme = useTheme()

console.log('App.vue loaded (Safe Imports Mode)')

const query = ref('')
const apps = ref([])
const files = ref([])
const selectedIndex = ref(0)
const appWindow = getCurrentWindow()

const config = ref(null)
const showSettings = ref(false)
const scripts = ref([])
const ollamaModels = ref([])
const fetchingModels = ref(false)

// AI Chat State
const showAiChat = ref(false)
const initialAiQuery = ref('')

// Load Apps, Scripts and Config on mount
onMounted(async () => {
  // 1. Load Apps
  try {
    apps.value = await invoke('list_apps')
    console.log('Apps loaded:', apps.value.length)
  } catch (e) {
    console.error('Failed to load apps', e)
  }

  // 2. Load Scripts
  try {
      scripts.value = await invoke('list_scripts')
      console.log('Scripts loaded:', scripts.value.length)
  } catch(e) {
      console.error('Failed to load scripts', e)
  }

  // 3. Load Config
  try {
    await reloadConfig()
  } catch(e) {
      console.error('Failed to load config', e)
  }
  
  // 4. Update Window Size (Initial)
  try {
     await updateWindowSize(false)
  } catch(e) {
      console.error('Failed to set initial size', e)
  }
})

async function reloadConfig() {
    config.value = await invoke('get_config')
    // Ensure shortcuts exists
    if (!config.value.shortcuts) config.value.shortcuts = {}
    
    if (config.value.theme) {
        applyTheme(config.value.theme)
        updateVuetifyTheme(config.value.theme)
    }
}

function updateVuetifyTheme(theme) {
    if (!theme) return
    vTheme.themes.value.dark.colors.primary = theme.primary
    vTheme.themes.value.dark.colors.secondary = theme.secondary
    vTheme.themes.value.dark.colors.background = theme.background
}

// Search Files
let searchTimeout
watch(query, (newVal) => {
  selectedIndex.value = 0
  if (!newVal) {
    files.value = []
    return
  }
  
  clearTimeout(searchTimeout)
  searchTimeout = setTimeout(async () => {
    try {
      files.value = await invoke('search_files', { query: newVal, path: '/home/jsteven' })
    } catch (e) {
      console.error(e)
    }
  }, 300)
})

const COLLAPSED_HEIGHT = 120
const EXPANDED_HEIGHT = 600
const BASE_WIDTH = 800
const CHAT_WIDTH = 400

// Tool Detection Logic
const matchedTool = computed(() => {
    if (!query.value) return null
    const q = query.value.toLowerCase()
    const tools = (config.value && config.value.ai_tools) || []
    const shortcuts = (config.value && config.value.shortcuts) || {}
    
    // 1. Check Shortcuts (Exact Match for trigger)
    if (shortcuts[q]) {
        const targetId = shortcuts[q]
        
        // Check if it's an app shortcut
        if (targetId.startsWith('app:')) {
            const exec = targetId.substring(4)
            const app = apps.value.find(a => a.exec === exec)
            if (app) {
                return {
                    type: 'app',
                    name: app.name,
                    description: 'Launch application',
                    icon: 'mdi-application',
                    data: app
                }
            }
        }
        
        // Otherwise it's a tool
        const tool = tools.find(t => t.id === targetId)
        if (tool) return { type: 'tool', ...tool }
    }
    
    // 2. Check Keywords (Prefix Match) - only for tools
    for (const tool of tools) {
        if (tool.keywords && tool.keywords.some(k => q.startsWith(k.toLowerCase()))) {
            return { type: 'tool', ...tool }
        }
    }
    
    // 3. Check SkillManager
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


// Dynamic Resizing
const currentBaseWidth = ref(BASE_WIDTH)

async function updateWindowSize(expanded) {
    try {
        const monitor = await currentMonitor()
        if (monitor) {
             const scaleFactor = monitor.scaleFactor
             const screenWidth = monitor.size.width / scaleFactor
             let targetWidth = Math.max(BASE_WIDTH, Math.floor(screenWidth * 0.5))
             currentBaseWidth.value = targetWidth
        } 
        
        let width = currentBaseWidth.value
        if (showAiChat.value) {
            width += CHAT_WIDTH + 8 // +8 for margin
        }
        
        const height = expanded ? EXPANDED_HEIGHT : COLLAPSED_HEIGHT
        
        // If chat is open, force expanded height
        const actualHeight = showAiChat.value ? EXPANDED_HEIGHT : height
        
        await appWindow.setSize(new LogicalSize(width, actualHeight))
    } catch (e) {
        console.error("Failed to resize window:", e)
    }
}

watch(query, async (newVal) => {
    if (!showAiChat.value) {
        if (newVal && newVal.length > 0) {
            await updateWindowSize(true)
        } else {
            await updateWindowSize(false)
        }
    }
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
}

async function executeAction(index) {
  let currentIndex = 0
  // 0: AI, Matched Tool, or App Shortcut
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
  currentIndex++
  
  // Apps
  if (index < currentIndex + filteredApps.value.length) {
    await executeApp(filteredApps.value[index - currentIndex])
    return
  }
  currentIndex += filteredApps.value.length
  
  // Scripts
  if (index < currentIndex + filteredScripts.value.length) {
    await executeScript(filteredScripts.value[index - currentIndex])
    return
  }
  currentIndex += filteredScripts.value.length

  // Files
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
        
        initialAiQuery.value = prompt
        showAiChat.value = true
        await updateWindowSize(true)
    } catch(e) {
        console.error("Failed to execute tool", e)
        initialAiQuery.value = "Failed to retrieve text context for this tool."
        showAiChat.value = true
        await updateWindowSize(true)
    }
}

async function executeSkill(match) {
    try {
        const result = await match.skill.execute(match.data)
        // If result is a string/number, determine what to do.
        // For now, copy to clipboard and notify/close
        if (result !== undefined && result !== null) {
            try {
                await invoke('copy_to_clipboard', { text: result.toString() })
                // Delay hiding to ensure clipboard transfer completes (Wayland race condition fix)
                await new Promise(resolve => setTimeout(resolve, 200))
            } catch (e) {
                console.error("Clipboard failed", e)
            }
            // Maybe show a toast? For now just close.
            await hideWindow()
        }
    } catch(e) {
        console.error("Failed to execute skill", e)
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

async function askAI() {
  if (!query.value) return
  initialAiQuery.value = query.value
  showAiChat.value = true
  await updateWindowSize(true)
}

function closeAiChat() {
    showAiChat.value = false
    initialAiQuery.value = ''
    if (query.value) {
        updateWindowSize(true)
    } else {
        updateWindowSize(false)
    }
}

function handleEsc() {
    if (showAiChat.value) {
        closeAiChat()
    } else {
        hideWindow()
    }
}

function handleConfigUpgrade(newConfig) {
    config.value = newConfig
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

async function hideWindow() {
  showAiChat.value = false
  await appWindow.hide()
}

function getFileName(path) {
  return path.split('/').pop()
}

</script>

<style>
/* Opaque Theme */
.glass-effect {
  background: var(--theme-background, #1e1e1e) !important;
  color: var(--theme-text, #ffffff);
}

.search-bar :deep(.v-field) {
    background-color: var(--theme-surface, #2d2d2d) !important;
}

.bg-surface-light {
    background-color: var(--theme-surface, rgba(255, 255, 255, 0.05)) !important;
}

.v-list-subheader {
    color: var(--theme-primary) !important;
    opacity: 0.8;
}

/* Hide scrollbar but keep functionality */
::-webkit-scrollbar {
  width: 6px;
}
::-webkit-scrollbar-track {
  background: transparent;
}
::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.2);
  border-radius: 3px;
}
</style>
