import { invoke } from '@tauri-apps/api/core'

export const backend = {
    async getConfig() {
        return await invoke('get_config')
    },

    async saveConfig(config) {
        return await invoke('save_config', { config })
    },

    async launchApp(execCmd) {
        return await invoke('launch_app', { execCmd })
    },

    async listApps() {
        return await invoke('list_apps')
    },

    async listWindows() {
        return await invoke('list_windows')
    },

    async focusWindow(address) {
        return await invoke('focus_window', { address })
    },

    async askAi(messages) {
        return await invoke('ask_ai', { messages })
    },

    async checkAiConnection() {
        return await invoke('check_ai_connection')
    },

    async listOllamaModels() {
        return await invoke('list_ollama_models')
    },

    async searchFiles(query, path) {
        return await invoke('search_files', { query, path })
    },

    async openEntity(path) {
        return await invoke('open_entity', { path })
    },

    async getSelectionContext() {
        return await invoke('get_selection_context')
    },

    async copyToClipboard(text) {
        return await invoke('copy_to_clipboard', { text })
    },

    async listScripts() {
        return await invoke('list_scripts')
    }
}
