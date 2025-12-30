<template>
  <div class="translation-container">
    <!-- Source Pane -->
    <div class="pane source-pane">
        <div class="lang-label" @click="openSelector('source')">
            <span class="lang-code">{{ sourceLangName || 'Detecting...' }}</span>
            <v-icon icon="mdi-chevron-down" size="small"></v-icon>
        </div>
        <textarea 
            ref="inputRef"
            v-model="internalQuery"
            class="text-input" 
            placeholder="Type to translate..."
            @keydown.tab.prevent="swapLanguages"
            @keydown.enter.prevent="handleEnter"
            spellcheck="false"
        ></textarea>
    </div>

    <div class="divider">
        <v-icon icon="mdi-arrow-right" size="small" color="grey"></v-icon>
    </div>

    <!-- Target Pane -->
    <div class="pane target-pane">
        <div class="lang-label" @click="openSelector('target')">
            <span class="lang-code">{{ targetLangName }}</span>
            <v-icon icon="mdi-chevron-down" size="small"></v-icon>
        </div>
        <div class="text-output" :class="{'placeholder': !translation}">
            <span v-if="loading" class="loading-pulse">Translating...</span>
            <span v-else>{{ translation || 'Translation' }}</span>
        </div>
        
        <!-- Copied Overlay -->
        <transition name="fade">
            <div v-if="showCopiedFeedback" class="copied-overlay">
                <v-icon icon="mdi-check-circle" color="#4ade80" size="64"></v-icon>
                <div class="copied-text">Copied!</div>
            </div>
        </transition>
    </div>
    
    <!-- Language Selector Overlay -->
    <div v-if="isSelecting" class="language-selector-overlay" @click.self="closeSelector">
        <div class="selector-card">
            <div class="selector-header">
                <v-icon icon="mdi-magnify" color="grey"></v-icon>
                <input 
                    ref="selectorSearchInput"
                    v-model="selectorSearch"
                    type="text" 
                    class="selector-input" 
                    placeholder="Search language..."
                    @keydown.esc="closeSelector"
                >
            </div>
            <div class="selector-list">
                <div 
                    v-for="lang in filteredLanguages" 
                    :key="lang.code"
                    class="selector-item"
                    :class="{'active': (isSelecting === 'source' ? sourceLang : targetLang) === lang.code}"
                    @click="selectLanguage(lang.code)"
                >
                    {{ lang.name }}
                </div>
            </div>
        </div>
    </div>
  </div>
  
  <!-- Footer Hints -->
  <div class="hints-footer">
    <div class="hint-item">
        <span class="key">↵</span>
        <span class="label">Copy & Close</span>
    </div>
    <div class="hint-item">
        <span class="key">Tab</span>
        <span class="label">Swap</span>
    </div>
    <div class="hint-item">
        <span class="key">Esc</span>
        <span class="label">Cancel</span>
    </div>
  </div>
</template>

<script setup>
import { ref, watch, onMounted, computed, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { readText } from '@tauri-apps/plugin-clipboard-manager'
import { useOmnibar } from '../../composables/useOmnibar'
import { LANGUAGES } from './languages'

const emit = defineEmits(['close'])
const { query, hideWindow } = useOmnibar()

const inputRef = ref(null)
const selectorSearchInput = ref(null)

const internalQuery = ref('')
const translation = ref('')
const loading = ref(false)
const showCopiedFeedback = ref(false)

// Selector State
const isSelecting = ref(false) // 'source' | 'target' | false
const selectorSearch = ref('')

const sourceLang = ref('auto')
const targetLang = ref('es') 
const userLocale = navigator.language.split('-')[0]
targetLang.value = userLocale === 'en' ? 'es' : userLocale 

const sourceLangName = computed(() => {
    const l = LANGUAGES.find(l => l.code === sourceLang.value)
    return l ? l.name : sourceLang.value
})

const targetLangName = computed(() => {
    const l = LANGUAGES.find(l => l.code === targetLang.value)
    return l ? l.name : targetLang.value
})

const filteredLanguages = computed(() => {
    let list = LANGUAGES
    if (isSelecting.value === 'target') {
        list = list.filter(l => !l.sourceOnly)
    }
    
    if (!selectorSearch.value) return list
    
    const q = selectorSearch.value.toLowerCase()
    return list.filter(l => l.name.toLowerCase().includes(q) || l.code.toLowerCase().includes(q))
})

function openSelector(side) { // 'source' or 'target'
    console.log('Opening selector for:', side)
    isSelecting.value = side
    selectorSearch.value = ''
    nextTick(() => {
        if (selectorSearchInput.value) {
            selectorSearchInput.value.focus()
        } else {
            console.warn('Selector input ref not found')
        }
    })
}

function selectLanguage(code) {
    if (isSelecting.value === 'source') {
        sourceLang.value = code
    } else if (isSelecting.value === 'target') {
        targetLang.value = code
    }
    isSelecting.value = false
    
    // Retranslate
    if (internalQuery.value) {
        performTranslation(internalQuery.value)
    }
    
    nextTick(() => {
        if (inputRef.value) inputRef.value.focus()
    })
}

function closeSelector() {
    isSelecting.value = false
    nextTick(() => {
        if (inputRef.value) inputRef.value.focus()
    })
}

// Sync with global query (stripping 'tr ')
watch(query, (newVal) => {
    if (newVal.startsWith('tr ')) {
        const content = newVal.substring(3)
        if (content !== internalQuery.value) {
            internalQuery.value = content
        }
    } else if (newVal === 'tr' || newVal === 'tr ') {
        internalQuery.value = ''
    }
}, { immediate: true })

// Watch internal input to trigger translation
let debounceTimer = null

watch(internalQuery, (newVal) => {
    // Update global query to keep in sync if needed (optional, but good for consistency)
    // But modifying query might re-trigger basic search watchers.
    // We are in 'translating' state, so basic search watchers in useOmnibar should be wary.
    
    if (!newVal.trim()) {
        translation.value = ''
        loading.value = false
        return
    }

    loading.value = true
    clearTimeout(debounceTimer)
    debounceTimer = setTimeout(() => {
        performTranslation(newVal)
    }, 500)
})

async function performTranslation(text) {
    try {
        const res = await invoke('translate', {
            text,
            sourceLang: sourceLang.value === 'auto' ? undefined : sourceLang.value,
            targetLang: targetLang.value
        })
        
        translation.value = res.translated_text
        if (res.source_language) {
            // Update displayed source language if detected
            // But we keep 'auto' in sourceLang ref for logic unless explicitly set?
            // Actually UX says "Detected: English".
            if (sourceLang.value === 'auto') {
                // We don't change sourceLang to 'en', we just show 'English' in UI?
                // The API returns detected.
                // I'll make a separate 'detectedLang' ref for display.
                // But simplifying:
            }
        }
        loading.value = false
    } catch (e) {
        console.error(e)
        translation.value = "Error"
        loading.value = false
    }
}

async function handleEnter() {
    if (translation.value && !showCopiedFeedback.value) {
        // Show feedback
        showCopiedFeedback.value = true
        
        // Copy immediately
        await invoke('copy_to_clipboard', { text: translation.value })
        
        // Wait for visual feedback
        setTimeout(async () => {
            // Reset state for next time
            internalQuery.value = ''
            query.value = '' 
            // query change triggers watcher in useOmnibar -> sets uiState to idle
            
            await hideWindow()
            loading.value = false // ensure reset
            showCopiedFeedback.value = false
        }, 600)
    }
}

function swapLanguages() {
    // Basic swap: Target becomes source (if not auto), Source becomes target.
    // If source is auto, we need the *detected* language to swap.
    // For now, simpler implementation:
    const temp = sourceLang.value
    sourceLang.value = targetLang.value
    targetLang.value = temp === 'auto' ? 'en' : temp // Fallback 'en' if auto, or use detected if available
    
    // Trigger re-translation
    performTranslation(internalQuery.value)
}

onMounted(async () => {
    console.log('OmnibarTranslation mounted. Languages available:', LANGUAGES.length)
    nextTick(() => {
        if (inputRef.value) inputRef.value.focus()
    })
    
    // Check if we opened with empty query (just 'tr ')
    if (!internalQuery.value) {
        try {
            // readText may throw if clipboard is empty or non-text
            const clipboardText = await readText()
            if (clipboardText && clipboardText.trim()) {
                internalQuery.value = clipboardText
            }
        } catch (e) {
            // Ignore clipboard errors (empty, image, etc)
            console.debug('Clipboard read failed or empty (expected if empty):', e)
        }
    }
})
</script>

<style scoped>
.translation-container {
    position: relative; /* For overlays */
    display: flex;
    width: 100%;
    height: 100%;
    background: rgba(30, 30, 30, 0.95);
    border-radius: 12px;
    overflow: hidden;
    padding: 20px;
    padding-bottom: 50px; /* Space for footer */
    align-items: center;
}

.pane {
    position: relative; /* For copied overlay */
    flex: 1;
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: 10px;
}

.divider {
    width: 40px;
    display: flex;
    justify-content: center;
    align-items: center;
    height: 100%;
    opacity: 0.5;
}

.lang-label {
    font-size: 0.8rem;
    text-transform: uppercase;
    color: #888;
    margin-bottom: 10px;
    letter-spacing: 1px;
    font-weight: 600;
}

.text-input {
    width: 100%;
    flex: 1; /* Take remaining space */
    background: transparent;
    border: none;
    color: #fff;
    font-size: 1.5rem;
    resize: none;
    outline: none;
    font-family: inherit;
    margin-top: 10px;
}

.text-output {
    width: 100%;
    flex: 1; /* Take remaining space */
    font-size: 2rem; /* Larger than input */
    font-weight: 700;
    color: #4ade80; /* Neon Green or Primary Color */
    word-wrap: break-word;
    overflow-y: auto;
    display: flex;
    align-items: flex-start;
    margin-top: 10px;
}

.text-output.placeholder {
    color: #555;
    font-weight: 400;
}

.loading-pulse {
    animation: pulse 1.5s infinite;
}

@keyframes pulse {
    0% { opacity: 0.5; }
    50% { opacity: 1; }
    100% { opacity: 0.5; }
}

.hints-footer {
    position: absolute;
    bottom: 20px;
    right: 30px;
    display: flex;
    gap: 15px;
    opacity: 0.6;
    pointer-events: none;
}

.hint-item {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.8rem;
    color: #aaa;
}

.key {
    background: rgba(255,255,255,0.1);
    padding: 2px 6px;
    border-radius: 4px;
    font-family: monospace;
    font-weight: bold;
}

/* Copied Overlay */
.copied-overlay {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background: rgba(30, 30, 30, 0.9);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    z-index: 10;
    backdrop-filter: blur(4px);
}

.copied-text {
    font-size: 1.5rem;
    font-weight: bold;
    color: #4ade80;
    margin-top: 10px;
}

/* Transitions */
.fade-enter-active, .fade-leave-active {
  transition: opacity 0.3s;
}
.fade-enter-from, .fade-leave-to {
  opacity: 0;
}

/* Language Selector */
.language-selector-overlay {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    z-index: 20;
    /* backdrop-filter: blur(2px); Maybe too distracting */
    display: flex;
    justify-content: center;
    padding-top: 60px; /* Offset from top */
}

.selector-card {
    width: 300px;
    height: 400px;
    background: #252525;
    border: 1px solid #444;
    border-radius: 8px;
    box-shadow: 0 10px 30px rgba(0,0,0,0.5);
    display: flex;
    flex-direction: column;
    overflow: hidden;
}

.selector-header {
    padding: 10px;
    border-bottom: 1px solid #333;
    display: flex;
    align-items: center;
    gap: 8px;
}

.selector-input {
    background: transparent;
    border: none;
    color: #fff;
    font-size: 0.9rem;
    width: 100%;
    outline: none;
}

.selector-list {
    flex: 1;
    overflow-y: auto;
}

.selector-item {
    padding: 8px 12px;
    cursor: pointer;
    font-size: 0.9rem;
    color: #ccc;
    transition: background 0.2s;
}

.selector-item:hover {
    background: rgba(255,255,255,0.05);
    color: #fff;
}

.selector-item.active {
    background: rgba(74, 222, 128, 0.1); /* Green tint */
    color: #4ade80;
}

/* Update lang-label to show interactivity */
.lang-label {
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 5px;
    transition: color 0.2s;
    position: relative;
    z-index: 5;
}

.lang-label:hover {
    color: #fff;
}
</style>
