<template>
  <v-dialog :model-value="modelValue" @update:model-value="$emit('update:modelValue', $event)" fullscreen transition="dialog-bottom-transition">
    <v-card class="settings-card fill-height">
      <div class="d-flex fill-height">
        <!-- Sidebar -->
        <div class="settings-sidebar bg-grey-darken-4 border-r-thin" style="width: 250px; flex-shrink: 0;">
          <v-list density="compact" nav class="pt-4">
              <v-list-item 
                  prepend-icon="mdi-cog" 
                  title="General & AI" 
                  value="general" 
                  :active="activeTab === 'general'"
                  @click="activeTab = 'general'"
              ></v-list-item>
              <v-list-item 
                  prepend-icon="mdi-robot" 
                  title="AI Tools" 
                  value="tools" 
                  :active="activeTab === 'tools'"
                  @click="activeTab = 'tools'"
              ></v-list-item>
               <v-list-item 
                  prepend-icon="mdi-keyboard" 
                  title="Shortcuts" 
                  value="shortcuts" 
                  :active="activeTab === 'shortcuts'"
                  @click="activeTab = 'shortcuts'"
              ></v-list-item>
          </v-list>
          
          <div class="pa-2" style="position: absolute; bottom: 0; left: 0; right: 0;">
            <v-btn block color="red" variant="text" @click="$emit('update:modelValue', false)">Close</v-btn>
          </div>
        </div>

        <!-- Content -->
        <div class="flex-grow-1 overflow-y-auto pa-6">
        
        <!-- General / Model Settings -->
        <div v-if="activeTab === 'general'">
            <h2 class="text-h5 mb-4">AI Configuration</h2>
            <v-card class="bg-surface-light rounded-xl mb-4" flat>
                <v-card-text>
                    <v-select
                        v-model="config.preferred_model"
                        label="Preferred AI Provider"
                        :items="['local', 'cloud']"
                        variant="outlined"
                        bg-color="grey-darken-4"
                    ></v-select>
                    
                    <v-expand-transition>
                        <div v-if="config.preferred_model === 'local'">
                            <v-text-field
                            v-model="config.local_model_url"
                            label="Ollama URL"
                            hint="e.g. http://localhost:11434"
                            variant="outlined"
                            bg-color="grey-darken-4"
                            @blur="fetchOllamaModels"
                            ></v-text-field>
                            
                            <v-select
                                v-model="config.ollama_model"
                                :items="ollamaModels"
                                label="Select Model"
                                :loading="fetchingModels"
                                variant="outlined"
                                bg-color="grey-darken-4"
                                no-data-text="No models found"
                            >
                                <template v-slot:append-item> 
                                    <div class="pa-2">
                                        <v-btn block size="small" variant="text" @click="fetchOllamaModels">Refresh Models</v-btn>
                                    </div>
                                </template>
                            </v-select>
                        </div>
                        <div v-else>
                            <v-text-field
                            v-model="config.openai_api_key"
                            label="OpenAI API Key"
                            type="password"
                            variant="outlined"
                            bg-color="grey-darken-4"
                            ></v-text-field>
                        </div>
                    </v-expand-transition>
                </v-card-text>
            </v-card>
            <v-btn color="primary" @click="save">Save Changes</v-btn>
        </div>

        <!-- AI Tools Editor -->
        <div v-if="activeTab === 'tools'">
            <div class="d-flex align-center mb-4">
                <h2 class="text-h5">AI Tools</h2>
                <v-spacer></v-spacer>
                <v-btn prepend-icon="mdi-plus" color="primary" @click="openToolEditor(null)">New Tool</v-btn>
            </div>
            
            <div class="d-flex flex-wrap gap-4">
                <v-card 
                    v-for="(tool, i) in config.ai_tools" 
                    :key="i"
                    class="bg-surface-light rounded-xl mb-3 w-100"
                    flat
                >
                    <v-card-item>
                        <template v-slot:prepend>
                            <v-avatar color="primary" variant="tonal" rounded>
                                <v-icon :icon="tool.icon || 'mdi-robot'"></v-icon>
                            </v-avatar>
                        </template>
                        <v-card-title>{{ tool.name }}</v-card-title>
                        <v-card-subtitle>{{ tool.description }}</v-card-subtitle>
                        <template v-slot:append>
                             <v-btn icon="mdi-pencil" variant="text" size="small" @click="openToolEditor(tool, i)"></v-btn>
                             <v-btn icon="mdi-delete" variant="text" size="small" color="red" @click="deleteTool(i)"></v-btn>
                        </template>
                    </v-card-item>
                    <v-card-text class="pt-2">
                        <v-chip-group>
                            <v-chip v-for="kw in tool.keywords" :key="kw" size="x-small" label>{{ kw }}</v-chip>
                        </v-chip-group>
                    </v-card-text>
                </v-card>
            </div>
        </div>
        
        <!-- Shortcuts Editor -->
        <div v-if="activeTab === 'shortcuts'">
             <div class="d-flex align-center mb-4">
                <h2 class="text-h5">Shortcuts</h2>
                <v-spacer></v-spacer>
                <v-btn prepend-icon="mdi-plus" color="primary" @click="openShortcutEditor(null)">New Shortcut</v-btn>
            </div>
            
            <v-table class="bg-surface-light rounded-xl">
                <thead>
                    <tr>
                        <th class="text-left">Trigger</th>
                        <th class="text-left">Action</th>
                        <th class="text-right">Actions</th>
                    </tr>
                </thead>
                <tbody>
                    <tr v-for="(target, trigger) in config.shortcuts" :key="trigger">
                        <td><v-chip label color="secondary" class="font-mono">{{ trigger }}</v-chip></td>
                        <td>{{ getToolName(target) || target }}</td>
                        <td class="text-right">
                            <v-btn icon="mdi-delete" variant="text" size="small" color="red" @click="deleteShortcut(trigger)"></v-btn>
                        </td>
                    </tr>
                    <tr v-if="Object.keys(config.shortcuts || {}).length === 0">
                        <td colspan="3" class="text-center text-medium-emphasis">No shortcuts defined</td>
                    </tr>
                </tbody>
            </v-table>
        </div>
        </div>

      </div>
    </v-card>
    
    <!-- Tool Editor Dialog -->
    <template v-if="modelValue">
      <v-dialog v-model="toolEditor.show" max-width="600">
          <v-card class="rounded-xl glass-effect">
              <v-card-title>{{ toolEditor.isNew ? 'Create Tool' : 'Edit Tool' }}</v-card-title>
              <v-card-text>
                  <v-text-field v-model="toolEditor.data.name" label="Name" variant="outlined"></v-text-field>
                  <v-text-field v-model="toolEditor.data.description" label="Description" variant="outlined"></v-text-field>
                  <v-text-field v-model="toolEditor.data.icon" label="Icon (mdi-name)" variant="outlined" prepend-inner-icon="mdi-emoticon-outline"></v-text-field>
                  
                  <v-combobox
                      v-model="toolEditor.data.keywords"
                      label="Keywords (Press Enter to add)"
                      multiple
                      chips
                      variant="outlined"
                      hint="Words that trigger this tool (e.g. 'rephrase')"
                      persistent-hint
                  ></v-combobox>
                  
                  <v-textarea 
                      v-model="toolEditor.data.prompt_template" 
                      label="Prompt Template" 
                      variant="outlined" 
                      rows="6"
                      hint="Use {{selection}} to insert highlighted text."
                      persistent-hint
                      class="mt-4 font-mono"
                  ></v-textarea>
              </v-card-text>
              <v-card-actions>
                  <v-spacer></v-spacer>
                  <v-btn @click="toolEditor.show = false">Cancel</v-btn>
                  <v-btn color="primary" @click="saveTool">Save</v-btn>
              </v-card-actions>
          </v-card>
      </v-dialog>
      
      <!-- Shortcut Editor Dialog -->
      <v-dialog v-model="shortcutEditor.show" max-width="400">
          <v-card class="rounded-xl glass-effect">
              <v-card-title>New Shortcut</v-card-title>
               <v-card-text>
                  <v-text-field v-model="shortcutEditor.trigger" label="Trigger (e.g. 'rpt')" variant="outlined" autofocus></v-text-field>
                  <v-select 
                      v-model="shortcutEditor.target" 
                      :items="availableTargets" 
                      item-title="name"
                      item-value="id"
                      label="Map to Tool or App" 
                      variant="outlined"
                  ></v-select>
              </v-card-text>
               <v-card-actions>
                  <v-spacer></v-spacer>
                  <v-btn @click="shortcutEditor.show = false">Cancel</v-btn>
                  <v-btn color="primary" @click="saveShortcut">Save</v-btn>
              </v-card-actions>
          </v-card>
      </v-dialog>
    </template>
    
  </v-dialog>
</template>

<script setup>
import { ref, watch, onMounted, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const props = defineProps(['modelValue', 'initialConfig', 'apps'])
const emit = defineEmits(['update:modelValue', 'config-updated'])

const activeTab = ref('general')
const config = ref({ 
    preferred_model: 'local',
    ai_tools: [], 
    shortcuts: {},
    local_model_url: 'http://localhost:11434',
    openai_api_key: '',
    ollama_model: ''
})
const ollamaModels = ref([])
const fetchingModels = ref(false)

watch(() => props.initialConfig, (val) => {
    console.log('Settings: initialConfig changed:', val)
    if (val) {
        config.value = JSON.parse(JSON.stringify(val))
        // Ensure shortcuts exists
        if(!config.value.shortcuts) config.value.shortcuts = {}
        if(val && val.preferred_model === 'local') fetchOllamaModels()
    }
}, { deep: true, immediate: true })

onMounted(() => {
    console.log('Settings component mounted!')
    console.log('modelValue:', props.modelValue)
    console.log('initialConfig:', props.initialConfig)
    console.log('apps:', props.apps?.length)
})

const availableTools = computed(() => {
    return (config.value.ai_tools || []).map(t => ({ name: t.name, id: t.id }))
})

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
    if(confirm('Are you sure?')) {
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
    
    console.log('Saving shortcut:', shortcutEditor.value.trigger, '->', shortcutEditor.value.target)
    console.log('Config before save:', JSON.stringify(config.value.shortcuts))
    
    // Force reactivity by creating new object
    config.value.shortcuts = {
        ...config.value.shortcuts,
        [shortcutEditor.value.trigger]: shortcutEditor.value.target
    }
    
    console.log('Config after update:', JSON.stringify(config.value.shortcuts))
    
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
    // Check if it's an app shortcut
    if (id.startsWith('app:')) {
        const exec = id.substring(4)
        const app = (props.apps || []).find(a => a.exec === exec)
        return app ? `📱 ${app.name}` : exec
    }
    // Otherwise it's a tool
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

async function save() {
    console.log('Saving config to backend:', JSON.stringify(config.value, null, 2))
    try {
        await invoke('save_config', { config: config.value })
        console.log('Config saved successfully')
        emit('config-updated', config.value)
    } catch (e) {
        console.error('Failed to save config:', e)
    }
}
</script>

<style scoped>
.settings-card {
    background: #1e1e1e !important;
}
.settings-sidebar {
    position: relative;
    height: 100%;
}
.border-r-thin {
    border-right: 1px solid rgba(255,255,255,0.1);
}
.font-mono {
    font-family: monospace;
}
</style>
