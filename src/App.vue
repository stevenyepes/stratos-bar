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
import { onMounted, onUnmounted } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import Settings from './components/Settings.vue'
import OmnibarSearch from './components/omnibar/OmnibarSearch.vue'
import OmnibarChat from './components/omnibar/OmnibarChat.vue'
import OmnibarTerminal from './components/omnibar/OmnibarTerminal.vue'

import { useOmnibar } from './composables/useOmnibar'
import { useAI } from './composables/useAI'
import { useScriptRunner } from './composables/useScriptRunner'

// Composables
const { 
  uiState, config, apps, showSettings,
  updateWindowSize, hideWindow, loadData, reloadConfig 
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
  
  window.addEventListener('keydown', handleGlobalKeydown)
  window.addEventListener('reload-config', reloadConfig)
})

onUnmounted(() => {
  window.removeEventListener('keydown', handleGlobalKeydown)
  window.removeEventListener('reload-config', reloadConfig)
  
  cleanupAiListeners()
  cleanupScriptListeners()
})

// Global Handlers
function handleGlobalKeydown(e) {
  if (e.key === 'Escape') {
    handleEsc()
  }
  
  if ((e.ctrlKey || e.metaKey) && e.key === ',') {
    e.preventDefault()
    showSettings.value = true
  }
}

function handleEsc() {
  if (showSettings.value) {
    showSettings.value = false
    return
  }
  if (uiState.value === 'chatting') {
    closeAiChat()
  } else if (uiState.value === 'executing') {
    closeTerminal() 
  } else {
    hideWindow()
  }
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
