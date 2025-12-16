<template>
  <v-card class="d-flex flex-column fill-height rounded-xl glass-effect" elevation="0">
    <!-- Header -->
    <v-card-title class="d-flex align-center py-2 px-4 bg-surface-light border-b-thin">
       <v-icon icon="mdi-robot" color="primary" class="mr-2" size="small"></v-icon>
       <span class="text-subtitle-2">AI Assistant</span>
       <v-spacer></v-spacer>
       <v-btn icon="mdi-close" variant="text" size="small" density="comfortable" @click="$emit('close')"></v-btn>
    </v-card-title>

    <!-- Messages -->
    <v-card-text class="flex-grow-1 overflow-y-auto pa-4" ref="messagesContainer" id="messages-container">
       <div v-if="messages.length === 0" class="text-center text-medium-emphasis mt-10 text-caption">
          Start a conversation...
       </div>
       <div v-for="(msg, i) in messages" :key="i" class="mb-4">
          <div class="text-caption font-weight-bold mb-1" :class="msg.role === 'user' ? 'text-right text-primary' : 'text-left text-secondary'">
             {{ msg.role === 'user' ? 'You' : 'AI' }}
          </div>
          <div 
            class="pa-3 rounded-lg text-body-2"
            :class="msg.role === 'user' ? 'bg-primary-darken-1 ml-auto' : 'bg-grey-darken-3'"
            style="max-width: 90%; width: fit-content;"
          >
             <div 
                v-if="msg.role === 'assistant'" 
                v-html="renderMarkdown(msg.content)" 
                class="markdown-body"
                @click="handleMessageClick"
             ></div>
             <div v-else style="white-space: pre-wrap;">{{ msg.content }}</div>
          </div>
       </div>
       <div v-if="loading" class="text-left mt-2">
           <v-progress-circular indeterminate size="20" width="2" color="primary"></v-progress-circular>
       </div>
    </v-card-text>

    <!-- Input -->
    <div class="pa-3 pt-0">
       <v-text-field
         v-model="input"
         placeholder="Reply..."
         variant="solo-filled" 
         bg-color="grey-darken-4"
         hide-details
         rounded="pill"
         density="compact"
         :loading="loading"
         @keydown.enter.prevent="sendMessage"
         autofocus
       >
         <template v-slot:append-inner>
            <v-btn icon="mdi-send" variant="text" size="small" color="primary" @click="sendMessage" :disabled="!input.trim()"></v-btn>
         </template>
       </v-text-field>
    </div>
  </v-card>
</template>

<script setup>
import { ref, nextTick, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { marked } from 'marked'
import hljs from 'highlight.js'
import 'highlight.js/styles/atom-one-dark.css'

const props = defineProps({
  initialQuery: {
    type: String,
    default: ''
  }
})

const emit = defineEmits(['close'])

const messages = ref([])
const input = ref('')
const loading = ref(false)
const messagesContainer = ref(null)

// Configure marked with highlight.js and custom renderer
const renderer = new marked.Renderer()
renderer.code = ({ text, lang }) => {
    const validLang = !!(lang && hljs.getLanguage(lang))
    const highlighted = validLang ? hljs.highlight(text, { language: lang }).value : text
    const langLabel = lang ? lang : 'text'
    
    // We escape the text for the attribute to be safe
    const escapedText = text.replace(/"/g, '&quot;')
    
    return `
      <div class="code-block-wrapper rounded-lg overflow-hidden my-2 bg-grey-darken-4 border-thin">
        <div class="d-flex align-center px-3 py-1 bg-grey-darken-3 border-b-thin">
           <span class="text-caption text-grey-lighten-1 font-weight-bold font-mono">${langLabel}</span>
           <div class="v-spacer"></div>
           <button class="copy-btn text-caption text-primary font-weight-bold ml-auto cursor-pointer" data-code="${escapedText}">
              Copy
           </button>
        </div>
        <pre><code class="hljs ${lang}">${highlighted}</code></pre>
      </div>
    `
}

marked.use({ renderer })

onMounted(() => {
    if (props.initialQuery) {
        messages.value.push({ role: 'user', content: props.initialQuery })
        sendMessage(null, true) 
    }
})

async function sendMessage(e, skipUserAdd = false) {
    if ((!input.value.trim() && !skipUserAdd) || loading.value) return
    
    if (!skipUserAdd) {
        messages.value.push({ role: 'user', content: input.value })
        input.value = ''
    }
    
    loading.value = true
    scrollToBottom()

    try {
        const history = JSON.parse(JSON.stringify(messages.value))
        const response = await invoke('ask_ai', { messages: history })
        messages.value.push({ role: 'assistant', content: response })
    } catch(err) {
        console.error(err)
        messages.value.push({ role: 'assistant', content: "Error: " + err })
    } finally {
        loading.value = false
        scrollToBottom()
    }
}

function scrollToBottom() {
    nextTick(() => {
        // v-card-text renders a generic element, usually div
        if (messagesContainer.value) {
            const el = messagesContainer.value.$el || messagesContainer.value
            el.scrollTop = el.scrollHeight
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

async function handleMessageClick(event) {
    const btn = event.target.closest('.copy-btn')
    if (btn) {
        // Decode the attribute logic or just use the sibling code block content
        // data-code might be large or messy with quotes. 
        // Better: Find the pre code sibling.
        const wrapper = btn.closest('.code-block-wrapper')
        if (wrapper) {
            const codeEl = wrapper.querySelector('code')
            if (codeEl) {
                const codeText = codeEl.innerText // innerText preserves newlines usually
                try {
                    await navigator.clipboard.writeText(codeText)
                    
                    // Visual feedback
                    const originalText = btn.innerText
                    btn.innerText = 'Copied!'
                    btn.classList.add('text-success')
                    setTimeout(() => {
                       btn.innerText = originalText
                       btn.classList.remove('text-success')
                    }, 2000)
                } catch(e) {
                    console.error("Failed to copy", e)
                }
            }
        }
    }
}
</script>

<style scoped>
.border-b-thin {
    border-bottom: 1px solid rgba(255,255,255,0.05);
}
.border-thin {
  border: 1px solid rgba(255,255,255,0.1);
}
/* Ensure glass effect inherits or is redefined if needed */
.glass-effect {
  background: #1e1e1e !important; 
  border: 1px solid rgba(255, 255, 255, 0.1);
}

.font-mono {
    font-family: monospace;
}
.cursor-pointer {
    cursor: pointer;
}
</style>

<style>
/* Markdown Styles - unscoped to affect v-html content */
.markdown-body {
    font-size: 0.95rem;
    line-height: 1.5;
}
.markdown-body h1, .markdown-body h2, .markdown-body h3 {
    margin-top: 0.5em;
    margin-bottom: 0.5em;
    font-weight: 600;
}
.markdown-body p {
    margin-bottom: 0.5em;
}
.markdown-body pre {
    background: rgba(0,0,0,0.3);
    padding: 0.5em;
    border-radius: 4px;
    overflow-x: auto;
    margin-bottom: 0.5em;
}
.markdown-body code {
    background: rgba(0,0,0,0.3);
    padding: 2px 4px;
    border-radius: 3px;
    font-family: monospace;
}
.markdown-body pre code {
    background: transparent;
    padding: 0;
}
.markdown-body ul, .markdown-body ol {
    padding-left: 1.5em;
    margin-bottom: 0.5em;
}
.markdown-body a {
    color: #4CAF50;
    text-decoration: none;
}
.markdown-body a:hover {
    text-decoration: underline;
}
</style>
