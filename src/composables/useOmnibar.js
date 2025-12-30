import { ref, computed, watch, nextTick, shallowRef } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow, currentMonitor } from '@tauri-apps/api/window'
import { LogicalSize } from '@tauri-apps/api/dpi'
import * as path from '@tauri-apps/api/path'
import { applyTheme } from '../theme'
import SkillManager from '../skills'
import { useTheme } from 'vuetify'

const COLLAPSED_HEIGHT = 100
const EXPANDED_HEIGHT = 500
const CHAT_HEIGHT = 600
const BASE_WIDTH = 700

// Singleton state to ensure consistency if shared (though mostly used in App.vue)
// For now, we'll keep it as a standard composable function, but usually these are singletons in this type of app.
// I'll define refs outside implementation to make it a singleton or inside for scoped. 
// Given the app structure, singleton is probably safer for the global window state.

const uiState = ref('idle') // 'idle', 'searching', 'chatting', 'executing'
const query = ref('')
const config = ref(null)
const apps = shallowRef([]) // use shallowRef for large lists for performance
const windows = shallowRef([])
const files = shallowRef([])
const scripts = shallowRef([])
const recentActions = shallowRef([])
const selectedIndex = ref(0)
const showSettings = ref(false)
const searchInput = ref(null) // Template ref

export function useOmnibar() {
    const appWindow = getCurrentWindow()
    const vTheme = useTheme()


    async function updateWindowSize() {
        try {
            let width = BASE_WIDTH
            let height = COLLAPSED_HEIGHT

            const monitor = await currentMonitor()

            if (monitor) {
                const scaleFactor = monitor.scaleFactor
                const screenWidth = monitor.size.width / scaleFactor
                width = Math.max(BASE_WIDTH, Math.floor(screenWidth * 0.4))
            } else {
                const webScreenWidth = window.screen.width
                if (webScreenWidth) {
                    width = Math.max(BASE_WIDTH, Math.floor(webScreenWidth * 0.4))
                }
            }

            if (uiState.value === 'chatting') {
                height = CHAT_HEIGHT
            } else if (uiState.value === 'searching') {
                height = EXPANDED_HEIGHT
                // Check if file search mode
                if (query.value && query.value.trim().toLowerCase().startsWith('ff ')) {
                    // Dual pane width
                    width = Math.max(1000, Math.floor(width * 1.4))
                }
            } else if (uiState.value === 'executing') {
                height = EXPANDED_HEIGHT
            }

            await appWindow.setSize(new LogicalSize(width, height))

            // Try to set focus immediately to combat resize blur
            if (searchInput.value) searchInput.value.focus()

            // Restore focus (delayed safety net)
            if (uiState.value === 'searching' || uiState.value === 'idle') {
                setTimeout(async () => {
                    if (searchInput.value) searchInput.value.focus()
                    await appWindow.setFocus()
                }, 150)
            }
        } catch (e) {
            console.error('Failed to resize window:', e)
        }
    }

    async function hideWindow() {
        await appWindow.hide()
    }

    async function focusWindow(win) {
        try {
            await invoke('focus_window', { address: win.address })
            query.value = ''
            await hideWindow()
        } catch (e) {
            console.error('Failed to focus window', e)
        }
    }

    async function reloadConfig() {
        config.value = await invoke('get_config')
        if (!config.value.shortcuts) config.value.shortcuts = {}

        if (config.value.theme) {
            applyTheme(config.value.theme)
            if (vTheme.themes.value && vTheme.themes.value.dark) {
                vTheme.themes.value.dark.colors.primary = config.value.theme.primary
                vTheme.themes.value.dark.colors.secondary = config.value.theme.secondary
            }
        }

        if (config.value.scripts) {
            scripts.value = config.value.scripts
        }
    }

    async function loadData() {
        try {
            await reloadConfig()
            const [appsList, scriptsList] = await Promise.all([
                invoke('list_apps'),
                invoke('list_scripts'),
                loadRecentActions()
            ])
            apps.value = appsList
            scripts.value = scriptsList
        } catch (e) {
            console.error('Failed to load data', e)
        }
    }

    async function loadRecentActions() {
        try {
            recentActions.value = await invoke('get_recent_actions', { limit: 20 })
        } catch (e) {
            console.error('Failed to load recent actions', e)
        }
    }

    async function recordAction(item) {
        try {
            if (!item) return;

            let action = {
                id: '',
                kind: 'app',
                content: '',
                name: '',
                last_accessed: Date.now(),
                frequency: 1
            };

            if (item.exec) { // App
                action.id = 'app:' + item.exec;
                action.kind = 'app';
                action.content = item.exec;
                action.name = item.name;
                action.icon = item.icon; // Added icon
            } else if (item.alias) { // Script
                action.id = 'script:' + item.alias;
                action.kind = 'script';
                action.content = item.path;
                action.name = item.alias;
            } else if (item.address) { // Window
                // We might not want to record window switching as persistent "Action" 
                // effectively, but user asked for "recent selected actions".
                // Window switching is ephemeral.
                // Let's exclude window switching for now unless requested.
                return;
            } else if (typeof item === 'string') { // File path
                action.id = 'file:' + item;
                action.kind = 'file';
                action.content = item;
                action.name = item.split('/').pop();
            } else if (item.type === 'tool' || item.type === 'skill') {
                action.id = 'ai:' + (item.id || item.name);
                action.kind = 'ai';
                action.content = item.id || item.name;
                action.name = item.name;
            } else {
                return; // Unknown
            }

            await invoke('record_action', { action })
            // Refresh
            loadRecentActions()
        } catch (e) {
            console.error('Failed to record action', e)
        }
    }

    // --- Computed Props ---

    const matchedTool = computed(() => {
        if (!query.value) return null
        const q = query.value.toLowerCase()
        const tools = (config.value && config.value.ai_tools) || []
        const shortcuts = (config.value && config.value.shortcuts) || {}

        if (shortcuts[q]) {
            const targetId = shortcuts[q]
            if (targetId.startsWith('app:')) {
                const exec = targetId.substring(4)
                const app = apps.value.find(a => a.exec === exec)
                if (app) {
                    return {
                        type: 'app',
                        name: app.name,
                        description: 'Launch application',
                        icon: app.icon || '🚀',
                        data: app
                    }
                }
            }
            const tool = tools.find(t => t.id === targetId)
            if (tool) return { type: 'tool', ...tool }
        }

        // Check scripts for exact match (Prioritized)
        const exactScript = scripts.value.find(s => s.alias.toLowerCase() === q)
        if (exactScript) {
            return {
                type: 'script',
                name: exactScript.alias,
                description: `Run script: ${exactScript.alias}`,
                icon: '💻',
                data: exactScript
            }
        }

        for (const tool of tools) {
            if (tool.keywords && tool.keywords.some(k => q.startsWith(k.toLowerCase()))) {
                return { type: 'tool', ...tool }
            }
        }

        const skillMatch = SkillManager.match(q)
        if (skillMatch) {
            return {
                type: 'skill',
                name: skillMatch.skill.name,
                description: skillMatch.preview || skillMatch.skill.description,
                icon: skillMatch.skill.icon,
                skill: skillMatch.skill,
                data: skillMatch.data
            }
        }

        if ('settings'.includes(q) && q.length > 1) {
            return {
                type: 'internal',
                id: 'settings',
                name: 'Open Settings',
                description: 'Configure appearance, shortcuts, and AI',
                icon: '⚙️'
            }
        }

        return null
    })

    const filteredWindows = computed(() => {
        if (!query.value) return []
        return windows.value.filter(w =>
            w.title.toLowerCase().includes(query.value.toLowerCase()) ||
            w.class.toLowerCase().includes(query.value.toLowerCase())
        ).slice(0, 5)
    })

    const filteredApps = computed(() => {
        if (!query.value) return []
        const q = query.value.toLowerCase()

        // 1. Filter
        let matches = apps.value.filter(app =>
            app.name.toLowerCase().includes(q) ||
            app.exec.toLowerCase().includes(q)
        )

        // 2. Rank using history
        const recentMap = new Map()
        recentActions.value.forEach((action, index) => {
            if (action.kind === 'app') {
                recentMap.set(action.content, 10000 - index)
            }
        })

        matches.sort((a, b) => {
            const scoreA = recentMap.get(a.exec) || 0
            const scoreB = recentMap.get(b.exec) || 0
            if (scoreA !== scoreB) return scoreB - scoreA // Descending score

            // Secondary sort: Starts with query?
            const aStarts = a.name.toLowerCase().startsWith(q)
            const bStarts = b.name.toLowerCase().startsWith(q)
            if (aStarts && !bStarts) return -1
            if (!aStarts && bStarts) return 1

            return a.name.localeCompare(b.name)
        })

        return matches.slice(0, 5)
    })

    const filteredScripts = computed(() => {
        if (!query.value) return scripts.value
        const q = query.value.toLowerCase()

        let matches = scripts.value.filter(s => s.alias.toLowerCase().includes(q))

        const recentMap = new Map()
        recentActions.value.forEach((action, index) => {
            if (action.kind === 'script') {
                // script actions store path in content, or unique alias?
                // logic in recordAction used item.path for content.
                // Assuming reliable mapping.
                recentMap.set(action.content, 10000 - index)
            }
        })

        matches.sort((a, b) => {
            const scoreA = recentMap.get(a.path) || 0
            const scoreB = recentMap.get(b.path) || 0
            if (scoreA !== scoreB) return scoreB - scoreA
            return a.alias.localeCompare(b.alias)
        })

        return matches
    })

    // Watchers
    watch(query, (newVal) => {
        // Smart selection
        if (matchedTool.value) {
            selectedIndex.value = 0
        } else {
            const hasWindows = windows.value.some(w =>
                w.title.toLowerCase().includes(newVal.toLowerCase()) ||
                w.class.toLowerCase().includes(newVal.toLowerCase())
            );
            const hasApps = apps.value.some(app =>
                app.name.toLowerCase().includes(newVal.toLowerCase()) ||
                app.exec.toLowerCase().includes(newVal.toLowerCase())
            );
            const hasScripts = scripts.value.some(s => s.alias.toLowerCase().includes(newVal.toLowerCase()));

            if (hasWindows || hasApps || hasScripts) {
                selectedIndex.value = 1
            } else {
                selectedIndex.value = 0
            }
        }

        if (newVal && uiState.value === 'idle') {
            uiState.value = 'searching'
            invoke('list_windows').then(w => windows.value = w).catch(e => console.error(e))
            updateWindowSize()
        } else if (!newVal && uiState.value === 'searching') {
            uiState.value = 'idle'
            updateWindowSize()
        }

        // File search
        if (!newVal || !newVal.toLowerCase().startsWith('ff ')) {
            files.value = []
        } else {
            const fileQuery = newVal.substring(3).trim()
            if (!fileQuery) {
                files.value = []
                return
            }
            clearTimeout(window.searchTimeout)
            window.searchTimeout = setTimeout(async () => {
                try {
                    const home = await path.homeDir()
                    files.value = await invoke('search_files', { query: fileQuery, path: home })
                } catch (e) {
                    console.error(e)
                }
            }, 300)
        }
    })

    async function clearActions() {
        try {
            await invoke('clear_history')
            recentActions.value = []
        } catch (e) {
            console.error('Failed to clear history', e)
        }
    }

    return {
        // State
        uiState,
        query,
        config,
        apps,
        windows,
        files,
        scripts,
        selectedIndex,
        showSettings,
        searchInput,

        // Computed
        matchedTool,
        filteredWindows,
        filteredApps,
        filteredScripts,

        // Actions
        updateWindowSize,
        hideWindow,
        focusWindow,
        reloadConfig,
        loadData,
        recordAction,
        clearActions,
        recentActions
    }
}
