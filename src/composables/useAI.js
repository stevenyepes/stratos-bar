import { ref, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useOmnibar } from './useOmnibar'

// Singleton State
const chatMessages = ref([])
const chatLoading = ref(false)
const chatInput = ref('')

export function useAI() {
    const { uiState, query, updateWindowSize, hideWindow } = useOmnibar()

    const chatInputElement = ref(null)
    const messagesContainer = ref(null)

    let unlisteners = []

    async function setupAiListeners() {
        unlisteners.push(await listen('ai-response-start', () => {
            chatLoading.value = false
            chatMessages.value.push({ role: 'assistant', content: '' })
            scrollToBottom()
        }))

        unlisteners.push(await listen('ai-response-chunk', (event) => {
            const lastMsg = chatMessages.value[chatMessages.value.length - 1]
            if (lastMsg && lastMsg.role === 'assistant') {
                lastMsg.content += event.payload
                scrollToBottom()
            }
        }))

        unlisteners.push(await listen('ai-response-done', () => {
            chatLoading.value = false
            scrollToBottom()
        }))

        unlisteners.push(await listen('ai-response-error', (event) => {
            chatMessages.value.push({ role: 'assistant', content: 'Error: ' + event.payload })
            scrollToBottom()
        }))
    }

    function cleanupAiListeners() {
        unlisteners.forEach(u => u())
        unlisteners = []
    }

    function scrollToBottom() {
        nextTick(() => {
            if (messagesContainer.value) {
                messagesContainer.value.scrollTop = messagesContainer.value.scrollHeight
            }
        })
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
            await invoke('ask_ai', { messages: history })
        } catch (err) {
            console.error(err)
            chatMessages.value.push({ role: 'assistant', content: 'Error: ' + err })
            chatLoading.value = false
            scrollToBottom()
        }
    }

    async function regenerateMessage(index) {
        if (chatLoading.value) return
        if (index > 0) {
            chatMessages.value = chatMessages.value.slice(0, index)
            chatLoading.value = true
            scrollToBottom()
            try {
                const history = JSON.parse(JSON.stringify(chatMessages.value))
                await invoke('ask_ai', { messages: history })
            } catch (err) {
                console.error(err)
                chatMessages.value.push({ role: 'assistant', content: 'Error: ' + err })
                chatLoading.value = false
                scrollToBottom()
            }
        }
    }

    function askAI() {
        if (!query.value) return
        chatMessages.value.push({ role: 'user', content: query.value })
        uiState.value = 'chatting'
        updateWindowSize()

        nextTick(() => {
            if (chatInputElement.value) chatInputElement.value.focus()
        })

        sendChatMessage(null, true)
    }

    function closeAiChat() {
        uiState.value = query.value ? 'searching' : 'idle'
        chatMessages.value = []
        chatInput.value = ''
        updateWindowSize()
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

            nextTick(() => {
                if (chatInputElement.value) chatInputElement.value.focus()
            })

            await sendChatMessage(null, true)
        } catch (e) {
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
        } catch (e) {
            console.error('Failed to execute skill', e)
        }
    }

    return {
        chatMessages,
        chatLoading,
        chatInput,
        chatInputElement,
        messagesContainer,
        setupAiListeners,
        cleanupAiListeners,
        sendChatMessage,
        regenerateMessage,
        askAI,
        closeAiChat,
        executeAiTool,
        executeSkill
    }
}
