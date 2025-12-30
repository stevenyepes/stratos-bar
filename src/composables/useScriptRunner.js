import { ref, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useOmnibar } from './useOmnibar'

// Singleton State
const scriptOutput = ref('')
const scriptRunning = ref(false)
const scriptError = ref(null)
const currentScript = ref(null)

export function useScriptRunner() {
    const { uiState, updateWindowSize } = useOmnibar()

    const terminalOutputRef = ref(null)

    let unlisteners = []

    async function setupScriptListeners() {
        unlisteners.push(await listen('script-start', () => {
            scriptRunning.value = true
            scriptOutput.value = ''
            scriptError.value = null
        }))

        unlisteners.push(await listen('script-output', (event) => {
            scriptOutput.value += event.payload
            nextTick(() => {
                if (terminalOutputRef.value) {
                    terminalOutputRef.value.scrollTop = terminalOutputRef.value.scrollHeight
                }
            })
        }))

        unlisteners.push(await listen('script-done', () => {
            scriptRunning.value = false
        }))
    }

    function cleanupScriptListeners() {
        unlisteners.forEach(u => u())
        unlisteners = []
    }

    async function executeScript(script) {
        try {
            currentScript.value = script
            uiState.value = 'executing'
            scriptError.value = null
            updateWindowSize()

            await invoke('execute_script', { path: script.path, args: script.args })
            const { recordAction } = useOmnibar()
            recordAction(script)

        } catch (e) {
            console.error(e)
            scriptError.value = e
            scriptRunning.value = false
        }
    }

    function closeTerminal() {
        uiState.value = 'idle'
        updateWindowSize()
    }

    return {
        scriptOutput,
        scriptRunning,
        scriptError,
        currentScript,
        terminalOutputRef, // expose for template ref
        setupScriptListeners,
        cleanupScriptListeners,
        executeScript,
        closeTerminal
    }
}
