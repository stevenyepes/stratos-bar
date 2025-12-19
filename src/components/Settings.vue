<template>
  <transition name="settings-slide">
    <div v-if="modelValue" class="settings-panel-overlay" @click.self="$emit('update:modelValue', false)">
      <div class="settings-panel">
        <div class="d-flex fill-height">

          <!-- Sidebar -->
          <div class="settings-sidebar d-flex flex-column pa-4">
            <div class="d-flex align-center mb-6 px-2 mt-2">
              <v-icon icon="mdi-cog" size="small" class="mr-2 text-primary"></v-icon>
              <span class="text-subtitle-2 font-weight-bold text-uppercase text-medium-emphasis">Settings</span>
            </div>

            <v-list density="comfortable" nav class="bg-transparent pa-0">
              <v-list-item 
                v-for="item in menuItems"
                :key="item.value"
                :value="item.value"
                :active="activeTab === item.value"
                @click="activeTab = item.value"
                class="mb-2 setting-nav-item"
              >
                <template v-slot:prepend>
                  <v-icon :icon="item.icon" size="20"></v-icon>
                </template>
                <v-list-item-title class="text-body-2">{{ item.title }}</v-list-item-title>
              </v-list-item>
            </v-list>
            
            <v-spacer></v-spacer>
          </div>

          <!-- Content -->
          <div class="settings-content flex-grow-1 d-flex flex-column bg-surface-darker">
            <!-- Header -->
            <div class="d-flex align-center px-8 py-5 border-b-thin bg-surface-header">
              <h2 class="text-h6 font-weight-medium">{{ activeTitle }}</h2>
              <v-spacer></v-spacer>
              <transition name="fade">
                <span v-if="showSaved" class="saved-indicator text-caption text-success mr-4">✓ Saved</span>
              </transition>
              <span class="text-caption text-disabled mr-4">v0.1.0</span>
              <v-btn variant="text" class="text-none" @click="$emit('update:modelValue', false)">Done</v-btn>
            </div>

            <div class="content-scroll-area flex-grow-1 overflow-y-auto px-8 py-6">
              <v-container class="pa-0" style="max-width: 1000px; margin: 0 auto;">
              <v-fade-transition mode="out-in">
                
                <!-- General / Model Settings -->
                <div v-if="activeTab === 'general'" key="general">
                  <div class="section-title mb-6">AI Configuration</div>
                  
                  <v-select
                    v-model="config.preferred_model"
                    label="Preferred AI Provider"
                    :items="['local', 'cloud']"
                    variant="underlined"
                    hide-details="auto"
                    class="mb-6 custom-input"
                    @update:model-value="autoSave"
                  ></v-select>
                  
                  <v-expand-transition>
                    <div v-if="config.preferred_model === 'local'">
                      <v-text-field
                        v-model="config.local_model_url"
                        label="Ollama Server URL"
                        placeholder="http://localhost:11434"
                        variant="underlined"
                        hide-details="auto"
                        class="mb-6 custom-input"
                        @blur="fetchOllamaModels"
                        @update:model-value="debouncedSave"
                      ></v-text-field>
                      
                      <div class="d-flex align-center gap-3 mb-6">
                        <v-select
                          v-model="config.ollama_model"
                          :items="ollamaModels"
                          label="Selected Model"
                          :loading="fetchingModels"
                          variant="underlined"
                          hide-details="auto"
                          class="flex-grow-1 custom-input"
                          no-data-text="No models found"
                          @update:model-value="autoSave"
                        ></v-select>
                        <v-btn 
                          icon="mdi-refresh" 
                          variant="text" 
                          size="small"
                          class="refresh-btn" 
                          :loading="fetchingModels"
                          @click="fetchOllamaModels"
                        ></v-btn>
                      </div>
                    </div>
                    <div v-else>
                      <v-text-field
                        v-model="config.openai_api_key"
                        label="OpenAI API Key"
                        type="password"
                        variant="underlined"
                        hide-details="auto"
                        class="custom-input mb-6"
                        placeholder="sk-..."
                        @update:model-value="debouncedSave"
                      ></v-text-field>
                    </div>
                  </v-expand-transition>
                </div>

                <!-- Appearance Settings -->
                <div v-if="activeTab === 'appearance'" key="appearance">
                  <div class="section-title mb-4">Theme Presets</div>
                  <div class="theme-grid mb-6">
                    <v-card 
                      v-for="preset in themePresets" 
                      :key="preset.name"
                      :class="['theme-preset-card', { 'active': config.theme?.name === preset.name && !config.theme?.is_custom }]"
                      @click="selectPreset(preset)"
                      flat
                    >
                       <div class="preset-preview" :style="{ background: preset.background }">
                          <div class="color-strips">
                             <div :style="{ background: preset.primary }"></div>
                             <div :style="{ background: preset.secondary }"></div>
                             <div :style="{ background: preset.surface }"></div>
                          </div>
                       </div>
                       <div class="pa-2 text-center text-caption font-weight-bold">{{ preset.name }}</div>
                    </v-card>
                  </div>

                  <div class="d-flex align-center mb-4">
                    <div class="section-title">Custom Colors</div>
                    <v-spacer></v-spacer>
                    <v-switch v-model="config.theme.is_custom" label="Enable Custom Colors" color="primary" hide-details density="compact" @update:model-value="autoSave"></v-switch>
                  </div>

                  <v-expand-transition>
                    <div v-if="config.theme?.is_custom">
                      <v-card class="setting-section-card border-thin mb-6" flat>
                        <v-card-text class="pa-4">
                           <div class="color-picker-grid">
                              <div v-for="color in colorFields" :key="color.key" class="d-flex align-center border-b-thin py-2">
                                 <div class="text-body-2">{{ color.label }}</div>
                                 <v-spacer></v-spacer>
                                 <div class="d-flex align-center">
                                    <span class="text-caption font-mono mr-2 text-medium-emphasis">{{ config.theme[color.key] }}</span>
                                    <input type="color" v-model="config.theme[color.key]" class="color-input" @input="debouncedSave">
                                 </div>
                              </div>
                           </div>
                        </v-card-text>
                      </v-card>
                    </div>
                  </v-expand-transition>
                </div>

                <!-- AI Tools Editor -->
                <div v-if="activeTab === 'tools'" key="tools">
                  <div class="d-flex align-center mb-4">
                     <div class="section-title">Enabled Tools</div>
                    <v-spacer></v-spacer>
                    <v-btn prepend-icon="mdi-plus" color="primary" variant="tonal" class="text-none" @click="openToolEditor(null)">Add Tool</v-btn>
                  </div>
                  
                  <div class="tool-grid">
                    <v-card 
                      v-for="(tool, i) in config.ai_tools" 
                      :key="i"
                      class="tool-card border-thin"
                      flat
                      @click="openToolEditor(tool, i)"
                    >
                      <div class="d-flex flex-column fill-height pa-4">
                        <div class="d-flex align-start mb-2">
                          <v-avatar color="primary" variant="tonal" rounded size="40" class="mr-3">
                            <v-icon :icon="tool.icon || 'mdi-robot'" size="24"></v-icon>
                          </v-avatar>
                          <div class="text-truncate">
                            <div class="text-subtitle-2 font-weight-bold text-truncate">{{ tool.name }}</div>
                            <div class="text-caption text-medium-emphasis text-truncate">{{ tool.description }}</div>
                          </div>
                          <v-spacer></v-spacer>
                        </div>
                        
                        <v-spacer></v-spacer>
                        
                        <div class="d-flex align-center mt-2 pt-2 border-t-thin">
                           <div class="d-flex gap-1 overflow-hidden mr-2">
                             <v-chip v-for="kw in (tool.keywords || []).slice(0,2)" :key="kw" size="x-small" density="comfortable" variant="flat" class="bg-surface-light">{{ kw }}</v-chip>
                             <span v-if="(tool.keywords || []).length > 2" class="text-caption text-disabled align-self-center">+{{ tool.keywords.length - 2 }}</span>
                           </div>
                           <v-spacer></v-spacer>
                           <v-btn icon="mdi-delete-outline" variant="text" size="small" density="compact" color="error" @click.stop="deleteTool(i)"></v-btn>
                        </div>
                      </div>
                    </v-card>
                  </div>
                </div>
                
                <!-- Shortcuts Editor -->
                <div v-if="activeTab === 'shortcuts'" key="shortcuts">
                  <div class="d-flex align-center mb-4">
                     <div class="section-title">Keyboard Shortcuts</div>
                    <v-spacer></v-spacer>
                    <v-btn prepend-icon="mdi-plus" color="primary" variant="tonal" class="text-none" @click="openShortcutEditor(null)">Add Shortcut</v-btn>
                  </div>
                  
                  <v-card class="border-thin overflow-hidden" flat>
                    <v-table class="bg-transparent hover-table">
                      <thead>
                        <tr class="bg-surface-light">
                          <th class="text-left font-weight-bold text-caption text-uppercase text-medium-emphasis">Trigger</th>
                          <th class="text-left font-weight-bold text-caption text-uppercase text-medium-emphasis">Action</th>
                          <th class="text-right font-weight-bold text-caption text-uppercase text-medium-emphasis" style="width: 80px">Action</th>
                        </tr>
                      </thead>
                      <tbody>
                        <tr v-for="(target, trigger) in config.shortcuts" :key="trigger">
                          <td>
                            <v-chip label size="small" variant="outlined" class="font-mono bg-surface-darker">{{ trigger }}</v-chip>
                          </td>
                          <td class="text-body-2">{{ getToolName(target) || target }}</td>
                          <td class="text-right">
                            <v-btn icon="mdi-delete-outline" variant="text" size="small" density="comfortable" color="medium-emphasis" @click="deleteShortcut(trigger)"></v-btn>
                          </td>
                        </tr>
                        <tr v-if="Object.keys(config.shortcuts || {}).length === 0">
                          <td colspan="3" class="text-center pa-8 text-medium-emphasis">
                            <v-icon icon="mdi-keyboard-off" size="large" class="mb-2 opacity-50"></v-icon>
                            <div>No shortcuts defined</div>
                          </td>
                        </tr>
                      </tbody>
                    </v-table>
                  </v-card>
                </div>

              </v-fade-transition>
              </v-container>
            </div>
          </div>
        </div>
      </div>
    </div>
  </transition>
    
    <!-- Tool Editor Dialog -->
    <v-dialog v-model="toolEditor.show" max-width="500" scrim="black opacity-80">
        <v-card class="rounded-xl border-thin bg-surface-dialog">
            <v-card-title class="px-6 pt-6 text-h6 font-weight-bold">{{ toolEditor.isNew ? 'Create Tool' : 'Edit Tool' }}</v-card-title>
            <v-card-text class="px-6 pt-4">
                <v-text-field v-model="toolEditor.data.name" label="Name" variant="outlined" density="comfortable" class="mb-3 custom-input"></v-text-field>
                <v-text-field v-model="toolEditor.data.description" label="Description" variant="outlined" density="comfortable" class="mb-3 custom-input"></v-text-field>
                <v-text-field v-model="toolEditor.data.icon" label="Icon (mdi-name)" variant="outlined" density="comfortable" prepend-inner-icon="mdi-emoticon-outline" class="mb-3 custom-input"></v-text-field>
                
                <v-combobox
                    v-model="toolEditor.data.keywords"
                    label="Keywords"
                    multiple
                    chips
                    closable-chips
                    variant="outlined"
                    density="comfortable"
                    hide-details="auto"
                    placeholder="Press Enter to add..."
                    class="mb-4 custom-input"
                ></v-combobox>
                
                <v-textarea 
                    v-model="toolEditor.data.prompt_template" 
                    label="Prompt Template" 
                    variant="outlined"
                    density="comfortable" 
                    rows="5"
                    hide-details="auto"
                    class="font-mono text-body-2 custom-input"
                    bg-color="surface-darker"
                ></v-textarea>
                <div class="text-caption text-medium-emphasis mt-2">
                  Use <code class="bg-surface-light px-1 rounded">@{{selection}}</code> to insert highlighted text.
                </div>
            </v-card-text>
            <v-card-actions class="px-6 pb-6 pt-2">
                <v-spacer></v-spacer>
                <v-btn variant="text" class="text-none" @click="toolEditor.show = false">Cancel</v-btn>
                <v-btn color="primary" variant="flat" class="px-6 text-none" @click="saveTool">Save Tool</v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>
      
    <!-- Shortcut Editor Dialog -->
    <v-dialog v-model="shortcutEditor.show" max-width="400" scrim="black opacity-80">
        <v-card class="rounded-xl border-thin bg-surface-dialog">
            <v-card-title class="px-6 pt-6 text-h6 font-weight-bold">New Shortcut</v-card-title>
              <v-card-text class="px-6 pt-4">
                <v-text-field 
                  v-model="shortcutEditor.trigger" 
                  label="Trigger Keyword" 
                  placeholder="e.g. 'sum'" 
                  variant="outlined" 
                  density="comfortable"
                  class="mb-4 custom-input"
                  autofocus
                ></v-text-field>
                <v-select 
                    v-model="shortcutEditor.target" 
                    :items="availableTargets" 
                    item-title="name"
                    item-value="id"
                    label="Action" 
                    variant="outlined"
                    density="comfortable"
                    class="custom-input"
                ></v-select>
            </v-card-text>
              <v-card-actions class="px-6 pb-6 pt-2">
                <v-spacer></v-spacer>
                <v-btn variant="text" class="text-none" @click="shortcutEditor.show = false">Cancel</v-btn>
                <v-btn color="primary" variant="flat" class="px-6 text-none" @click="saveShortcut">Add Shortcut</v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>
    
</template>

<script setup>
import { ref, watch, onMounted, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { themePresets, applyTheme } from '../theme'
import { useTheme } from 'vuetify'

const vTheme = useTheme()

const props = defineProps(['modelValue', 'initialConfig', 'apps'])
const emit = defineEmits(['update:modelValue', 'config-updated'])

const activeTab = ref('general')

const menuItems = [
  { title: 'General & AI', value: 'general', icon: 'mdi-cog-outline' },
  { title: 'Appearance', value: 'appearance', icon: 'mdi-palette-outline' },
  { title: 'AI Tools', value: 'tools', icon: 'mdi-robot-outline' },
  { title: 'Shortcuts', value: 'shortcuts', icon: 'mdi-keyboard-outline' }
]

const activeTitle = computed(() => {
  const item = menuItems.find(i => i.value === activeTab.value)
  return item ? item.title : 'Settings'
})

const config = ref({ 
    preferred_model: 'local',
    ai_tools: [], 
    shortcuts: {},
    local_model_url: 'http://localhost:11434',
    openai_api_key: '',
    ollama_model: '',
    theme: {
        name: 'Tokyo Night',
        primary: '#7aa2f7',
        secondary: '#bb9af7',
        background: '#1a1b26',
        surface: '#24283b',
        text: '#c0caf5',
        is_custom: false
    }
})
const ollamaModels = ref([])
const fetchingModels = ref(false)
const showSaved = ref(false)
let saveTimeout = null

watch(() => props.initialConfig, (val) => {
    console.log('Settings: initialConfig changed:', val)
    if (val) {
        config.value = JSON.parse(JSON.stringify(val))
        if(!config.value.shortcuts) config.value.shortcuts = {}
        if(val && val.preferred_model === 'local') fetchOllamaModels()
    }
}, { deep: true, immediate: true })

const availableTargets = computed(() => {
    const tools = (config.value.ai_tools || []).map(t => ({ 
        name: `🤖 ${t.name}`, 
        id: t.id,
        type: 'tool'
    }))
    const apps = (props.apps || []).map(app => ({
        name: `📱 ${app.name}`,
        id: `app:${app.exec}`,
        type: 'app'
    }))
    return [...tools, ...apps]
})

// Tool Editor
const toolEditor = ref({
    show: false,
    isNew: true,
    index: -1,
    data: { name: '', description: '', icon: '', keywords: [], prompt_template: '' }
})

function openToolEditor(tool, index) {
    if (tool) {
        toolEditor.value.isNew = false
        toolEditor.value.index = index
        toolEditor.value.data = JSON.parse(JSON.stringify(tool))
    } else {
        toolEditor.value.isNew = true
        toolEditor.value.index = -1
        let id = 'tool_' + Date.now()
        toolEditor.value.data = { id, name: '', description: '', icon: 'mdi-robot', keywords: [], prompt_template: '' }
    }
    toolEditor.value.show = true
}

function saveTool() {
    if (toolEditor.value.isNew) {
        config.value.ai_tools.push(toolEditor.value.data)
    } else {
        config.value.ai_tools[toolEditor.value.index] = toolEditor.value.data
    }
    toolEditor.value.show = false
    save()
}

function deleteTool(index) {
    if(confirm('Are you sure you want to delete this tool?')) {
        config.value.ai_tools.splice(index, 1)
        save()
    }
}

// Shortcut Editor
const shortcutEditor = ref({
    show: false,
    trigger: '',
    target: ''
})

function openShortcutEditor() {
    shortcutEditor.value.trigger = ''
    shortcutEditor.value.target = ''
    shortcutEditor.value.show = true
}

function saveShortcut() {
    if (!shortcutEditor.value.trigger || !shortcutEditor.value.target) return
    if (!config.value.shortcuts) config.value.shortcuts = {}
    
    config.value.shortcuts = {
        ...config.value.shortcuts,
        [shortcutEditor.value.trigger]: shortcutEditor.value.target
    }
    shortcutEditor.value.show = false
    save()
}

function deleteShortcut(trigger) {
    if (!config.value.shortcuts) config.value.shortcuts = {}
    const newShortcuts = { ...config.value.shortcuts }
    delete newShortcuts[trigger]
    config.value.shortcuts = newShortcuts
    save()
}

function getToolName(id) {
    if (id.startsWith('app:')) {
        const exec = id.substring(4)
        const app = (props.apps || []).find(a => a.exec === exec)
        return app ? `📱 ${app.name}` : exec
    }
    const t = config.value.ai_tools.find(x => x.id === id)
    return t ? `🤖 ${t.name}` : id
}

async function fetchOllamaModels() {
    if (config.value && config.value.preferred_model !== 'local') return
    fetchingModels.value = true
    try {
        await invoke('save_config', { config: config.value }) 
        ollamaModels.value = await invoke('list_ollama_models')
    } catch(e) {
        console.error("Failed to fetch models", e)
    } finally {
        fetchingModels.value = false
    }
}

const colorFields = [
    { label: 'Primary', key: 'primary' },
    { label: 'Secondary', key: 'secondary' },
    { label: 'Background', key: 'background' },
    { label: 'Surface', key: 'surface' },
    { label: 'Text', key: 'text' }
]

function selectPreset(preset) {
    config.value.theme = { ...preset, is_custom: false }
    applyTheme(config.value.theme)
    autoSave()
}


watch(() => config.value?.theme, (newTheme) => {
    if (newTheme) {
        applyTheme(newTheme)
        // Update Vuetify dynamic colors
        vTheme.themes.value.dark.colors.primary = newTheme.primary
        vTheme.themes.value.dark.colors.secondary = newTheme.secondary
        vTheme.themes.value.dark.colors.background = newTheme.background
    }
}, { deep: true })

async function save() {
    try {
        await invoke('save_config', { config: config.value })
        emit('config-updated', config.value)
        
        // Show saved indicator
        showSaved.value = true
        if (saveTimeout) clearTimeout(saveTimeout)
        saveTimeout = setTimeout(() => {
            showSaved.value = false
        }, 2000)
    } catch (e) {
        console.error('Failed to save config:', e)
    }
}

// Auto-save for immediate changes (dropdowns, switches)
async function autoSave() {
    await save()
}

// Debounced save for text inputs
let debounceTimeout = null
function debouncedSave() {
    if (debounceTimeout) clearTimeout(debounceTimeout)
    debounceTimeout = setTimeout(async () => {
        await save()
    }, 800)
}

</script>

<style scoped>
/* Settings Panel Overlay */
.settings-panel-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.7);
  backdrop-filter: blur(8px);
  z-index: 2000;
  display: flex;
  align-items: center;
  justify-content: center;
}

/* Settings Panel */
.settings-panel {
  width: 90%;
  max-width: 1200px;
  height: 85vh;
  max-height: 800px;
  background: var(--theme-background, #18181b);
  border-radius: 16px;
  overflow: hidden;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
  border: 1px solid rgba(255, 255, 255, 0.08);
  color: var(--theme-text, #ffffff);
}

/* Slide Transition */
.settings-slide-enter-active {
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
}

.settings-slide-leave-active {
  transition: all 0.2s cubic-bezier(0.4, 0, 1, 1);
}

.settings-slide-enter-from {
  opacity: 0;
}

.settings-slide-enter-from .settings-panel {
  transform: scale(0.95) translateY(20px);
  opacity: 0;
}

.settings-slide-leave-to {
  opacity: 0;
}

.settings-slide-leave-to .settings-panel {
  transform: scale(0.98) translateY(10px);
  opacity: 0;
}


.settings-sidebar {
  width: 260px;
  background: rgba(0, 0, 0, 0.2);
  border-right: 1px solid rgba(255, 255, 255, 0.05);
}

.settings-content {
  position: relative;
}

.bg-surface-darker {
  background: rgba(0, 0, 0, 0.1);
}

.bg-surface-header {
  background: rgba(255, 255, 255, 0.02);
}

.setting-section-card {
  background: rgba(255, 255, 255, 0.03) !important;
  border-color: rgba(255, 255, 255, 0.05) !important;
}

/* Tool Grid */
.tool-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 16px;
}

.tool-card {
  background: rgba(255, 255, 255, 0.03) !important;
  border-color: rgba(255, 255, 255, 0.05) !important;
  transition: all 0.2s ease;
  cursor: pointer;
  height: 100%;
}

.tool-card:hover {
  background: rgba(255, 255, 255, 0.06) !important;
  transform: translateY(-2px);
  border-color: rgba(var(--v-theme-primary), 0.3) !important;
}

/* Sidebar Navigation */
.setting-nav-item {
  position: relative;
  transition: all 0.2s ease;
  border-radius: 8px;
  margin-bottom: 4px;
}

.setting-nav-item:hover {
  background: rgba(255, 255, 255, 0.04);
}

.setting-nav-item:deep(.v-list-item__prepend) {
  margin-right: 12px;
}

/* Active state: subtle left border accent */
.setting-nav-item.v-list-item--active {
  background: rgba(255, 255, 255, 0.05);
}

.setting-nav-item.v-list-item--active::before {
  content: '';
  position: absolute;
  left: 0;
  top: 25%;
  bottom: 25%;
  width: 3px;
  background: rgb(var(--v-theme-primary));
  border-radius: 0 2px 2px 0;
}

.setting-nav-item.v-list-item--active :deep(.v-list-item-title) {
  font-weight: 600;
  color: rgba(255, 255, 255, 0.95);
}

/* Custom Inputs - Underlined Style */
:deep(.custom-input .v-field__outline) {
  display: none;
}

:deep(.custom-input .v-field__underlay) {
  background: transparent;
}

:deep(.custom-input .v-field) {
  border-bottom: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 0;
}

:deep(.custom-input.v-input--is-focused .v-field) {
  border-bottom-color: rgb(var(--v-theme-primary));
  border-bottom-width: 2px;
}

/* Refresh Button */
.refresh-btn {
  transition: transform 0.3s ease;
}

.refresh-btn:hover {
  transform: rotate(180deg);
}

/* Saved Indicator */
.saved-indicator {
  font-weight: 500;
  opacity: 0.9;
}

.fade-enter-active, .fade-leave-active {
  transition: opacity 0.3s ease;
}

.fade-enter-from, .fade-leave-to {
  opacity: 0;
}

.section-title {
  text-transform: uppercase;
  font-size: 0.75rem;
  letter-spacing: 1px;
  font-weight: 700;
  color: rgba(255, 255, 255, 0.5);
}

.border-thin {
  border: 1px solid rgba(255, 255, 255, 0.08) !important;
}

.font-mono {
  font-family: 'JetBrains Mono', 'Fira Code', monospace !important;
}

.gap-2 { gap: 8px; }
.gap-4 { gap: 16px; }

/* Scrollbar styling */
.content-scroll-area::-webkit-scrollbar {
  width: 8px;
}
.content-scroll-area::-webkit-scrollbar-track {
  background: transparent;
}
.content-scroll-area::-webkit-scrollbar-thumb {
  background-color: rgba(255, 255, 255, 0.1);
  border-radius: 4px;
}
.content-scroll-area::-webkit-scrollbar-thumb:hover {
  background-color: rgba(255, 255, 255, 0.2);
}

.bg-surface-dialog {
  background: var(--theme-surface, #1e1e1e) !important;
}

/* Theme Appearance Styles */
.theme-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  gap: 16px;
}

.theme-preset-card {
  background: rgba(255, 255, 255, 0.03) !important;
  border: 1px solid rgba(255, 255, 255, 0.05) !important;
  cursor: pointer;
  transition: all 0.2s ease;
  overflow: hidden;
}

.theme-preset-card:hover {
  background: rgba(255, 255, 255, 0.06) !important;
  transform: translateY(-2px);
}

.theme-preset-card.active {
  border-color: var(--theme-primary, #BB86FC) !important;
  background: rgba(var(--v-theme-primary), 0.1) !important;
}

.preset-preview {
  height: 60px;
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
}

.color-strips {
  display: flex;
  gap: 4px;
  padding: 8px;
  background: rgba(0,0,0,0.3);
  border-radius: 4px;
}

.color-strips > div {
  width: 12px;
  height: 12px;
  border-radius: 2px;
}

.color-input {
  -webkit-appearance: none;
  -moz-appearance: none;
  appearance: none;
  width: 32px;
  height: 32px;
  background-color: transparent;
  border: none;
  cursor: pointer;
}

.color-input::-webkit-color-swatch {
  border-radius: 50%;
  border: 2px solid rgba(255,255,255,0.2);
}

.color-picker-grid {
    display: flex;
    flex-direction: column;
}
</style>
