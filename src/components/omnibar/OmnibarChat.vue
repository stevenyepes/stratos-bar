<template>
  <div class="omnibar-chat-mode scale-in">
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
              <button class="action-btn interactive" @click="regenerateMessage(i)" title="Regenerate">
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
        ref="chatInputElement"
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
</template>

<script setup>
import { onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { marked } from 'marked'
import hljs from 'highlight.js'
import 'highlight.js/styles/atom-one-dark.css'
import { useAI } from '../../composables/useAI'
import { useOmnibar } from '../../composables/useOmnibar'

const { 
  chatMessages, chatLoading, chatInput, chatInputElement, messagesContainer,
  sendChatMessage, regenerateMessage, closeAiChat 
} = useAI()

const { showSettings } = useOmnibar()

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

function renderMarkdown(text) {
  try {
    return marked.parse(text)
  } catch (e) {
    return text
  }
}

async function copyMessage(content) {
  try {
    await invoke('copy_to_clipboard', { text: content })
  } catch(e) {
    console.error('Failed to copy', e)
  }
}

function handleCodeCopy(e) {
  if (e.target.classList.contains('code-copy-btn')) {
    const code = e.target.getAttribute('data-code')
    if (code) {
      invoke('copy_to_clipboard', { text: code })
        .then(() => {
          const originalText = e.target.innerText
          e.target.innerText = 'Copied!'
          setTimeout(() => {
            e.target.innerText = originalText
          }, 2000)
        })
        .catch(err => console.error('Failed to copy code:', err))
    }
  }
}

onMounted(() => {
  document.addEventListener('click', handleCodeCopy)
})

onUnmounted(() => {
  document.removeEventListener('click', handleCodeCopy)
})
</script>

<style scoped>
/* Chat Mode Styles from App.vue */
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
.flex-grow { flex-grow: 1; }
.ml-2 { margin-left: var(--space-2); }
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
