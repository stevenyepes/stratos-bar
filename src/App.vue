<template>
  <v-app theme="dark" style="height: 100vh; background: transparent;">
    <v-main class="pa-0" style="height: 100vh; background: transparent;">
      <div class="omnibar-container" :class="{'omnibar-expanded': uiState !== 'idle', 'omnibar-executing': uiState === 'executing'}">
        
        <!-- State 1: Idle / State 2: Searching -->
        <OmnibarSearch 
          v-if="uiState === 'idle' || uiState === 'searching'" 
          @close="handleEsc"
        />

        <!-- State 3: Chat Mode -->
        <OmnibarChat v-else-if="uiState === 'chatting'" />

        <!-- State 4: Script Execution Mode -->
        <OmnibarTerminal v-else-if="uiState === 'executing'" />

        <!-- State 5: Translation Mode -->
        <OmnibarTranslation 
           v-else-if="uiState === 'translating'"
           @close="handleEsc"
        />

        <!-- Settings Component (Overlay) -->
        <Settings 
          v-model="showSettings" 
          :initial-config="config"
          :apps="apps"
          @config-updated="reloadConfig"
        />

      </div>
    </v-main>
  </v-app>
</template>

<script setup>
import { onMounted, onUnmounted, nextTick } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { listen } from '@tauri-apps/api/event'
import Settings from './components/Settings.vue'
import OmnibarSearch from './components/omnibar/OmnibarSearch.vue'
import OmnibarChat from './components/omnibar/OmnibarChat.vue'
import OmnibarTerminal from './components/omnibar/OmnibarTerminal.vue'
import OmnibarTranslation from './components/omnibar/OmnibarTranslation.vue'

import { useOmnibar } from './composables/useOmnibar'
import { useAI } from './composables/useAI'
import { useScriptRunner } from './composables/useScriptRunner'

// Composables
const { 
  uiState, config, apps, showSettings,
  updateWindowSize, hideWindow, loadData, reloadConfig,
  searchInput, query
} = useOmnibar()

const { setupAiListeners, closeAiChat, cleanupAiListeners } = useAI()
const { setupScriptListeners, closeTerminal, cleanupScriptListeners } = useScriptRunner()

const appWindow = getCurrentWindow()

// Lifecycle
onMounted(async () => {
  try {
    // 1. Resize and show window immediately
    await updateWindowSize()
    await appWindow.show()
    await appWindow.setFocus()

    // 2. Load heavy data
    loadData()
    
    // 3. Setup listeners
    await setupAiListeners()
    await setupScriptListeners()
    
  } catch (e) {
    console.error('Failed to initialize', e)
    await appWindow.show()
  }
  
  await listen('window-shown', () => {
     handleWindowFocus()
  })
  
  window.addEventListener('keydown', handleGlobalKeydown)
  window.addEventListener('reload-config', reloadConfig)
  window.addEventListener('focus', handleWindowFocus)
})

onUnmounted(() => {
  window.removeEventListener('keydown', handleGlobalKeydown)
  window.removeEventListener('reload-config', reloadConfig)
  window.removeEventListener('focus', handleWindowFocus)
  
  cleanupAiListeners()
  cleanupScriptListeners()
})

// Global Handlers

function handleGlobalKeydown(e) {
  if (e.key === 'Escape') {
    handleEsc()
    return
  }
  
  if ((e.ctrlKey || e.metaKey) && e.key === ',') {
    e.preventDefault()
    showSettings.value = true
    return
  }

  // Fix for typing issue: If typing alphanumeric chars and focus is lost (body), focus input
  if (
    !e.ctrlKey && !e.metaKey && !e.altKey && 
    e.key.length === 1 && 
    (document.activeElement === document.body || !document.activeElement)
  ) {
    if (uiState.value === 'idle' || uiState.value === 'searching') {
        if (searchInput.value) {
            searchInput.value.focus()
            // Rescue the lost key by manually appending it if it wasn't captured by input
            query.value += e.key
            e.preventDefault()
        }
    }
  }
}

function handleEsc() {
  if (showSettings.value) {
    // Check for open dialogs (Vuetify overlays) inside settings
    // If a dialog is open, let Vuetify handle the ESC (closing the dialog)
    // and do not close the entire settings panel.
    if (document.querySelector('.v-overlay--active')) {
      return
    }

    showSettings.value = false
    focusInput()
    return
  }
  if (uiState.value === 'chatting') {
    closeAiChat()
  } else if (uiState.value === 'executing') {
    closeTerminal() 
  } else if (uiState.value === 'translating') {
      // Go back to launcher (idle state)
      query.value = ''
      focusInput()
  } else {
    hideWindow()
  }
}

async function handleWindowFocus() {
  // If settings are open, don't steal focus to search
  if (showSettings.value) return

  if (uiState.value === 'idle' || uiState.value === 'searching') {
    focusInput()
    // Retry shortly after to handle potential window animation/transition delays
    setTimeout(focusInput, 50)
  }
}

function focusInput() {
  nextTick(() => {
    if (searchInput.value) searchInput.value.focus()
  })
}
</script>

<style scoped>
/* Container Styles */
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
</style>

<style>
/* Global effects needed for neon glow */
.omnibar-executing {
    box-shadow: 0 0 0 1px #4ade80, 0 0 20px rgba(74, 222, 128, 0.4) !important;
    border-color: #4ade80 !important;
}
</style>
