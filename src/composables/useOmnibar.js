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
const BASE_WIDTH = 600

const SEARCH_DEBOUNCE_MS = 90
const FREQUENT_APP_LIMIT = 8

export const KIND_FILTER_OPTIONS = [
    { id: 'all', label: 'All', kinds: null },
    { id: 'apps', label: 'Apps', kinds: ['app'] },
    { id: 'scripts', label: 'Scripts', kinds: ['script'] },
    { id: 'files', label: 'Files', kinds: ['recent_file'] },
    { id: 'recent', label: 'Recent', kinds: ['shortcut'] },
]

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

const scoredApps = shallowRef([])
const topFrecencyApps = shallowRef([])
const aliasesByAppId = ref({})
const kindFilter = ref('all')
const discoverResults = shallowRef([])

let searchDebounce = null
let lastSearchToken = 0
let queryWatcherInstalled = false
function runSharedSearch(value) {
    const trimmed = (value || '').trim()
    if (!trimmed) {
        discoverResults.value = []
        scoredApps.value = []
        return
    }
    const token = ++lastSearchToken
    const option = KIND_FILTER_OPTIONS.find((o) => o.id === kindFilter.value) || KIND_FILTER_OPTIONS[0]
    const kinds = option && option.kinds ? option.kinds : null
    const payload = { query: trimmed, limit: 25 }
    if (kinds) payload.kinds = kinds
    invoke('search_discoverable', payload)
        .then((result) => {
            if (token !== lastSearchToken) return
            const list = Array.isArray(result) ? result : []
            discoverResults.value = mergeAliasesIntoResults(list)
            scoredApps.value = discoverResults.value
                .filter((d) => d && d.kind === 'app')
                .map((d) => ({ app: discoverableToApp(d), score: d.score }))
        })
        .catch((e) => {
            console.error('search_discoverable failed', e)
            discoverResults.value = []
            scoredApps.value = []
        })
}

function discoverableToApp(d) {
    if (!d) return null
    const exec = d.launch && typeof d.launch.exec === 'string' ? d.launch.exec : ''
    const source = typeof d.source === 'string' && d.source.startsWith('app:')
        ? d.source.slice(4)
        : (d.source || '')
    return {
        id: d.id,
        name: d.name,
        exec,
        icon: d.icon || null,
        source,
        categories: d.category ? [d.category] : [],
        description: d.description || null,
        aliases: Array.isArray(d.aliases) ? d.aliases : [],
    }
}

function discoverableToScript(d) {
    if (!d) return null
    return {
        id: d.id,
        alias: d.name,
        path: d.launch && typeof d.launch.path === 'string' ? d.launch.path : '',
        args: d.launch && typeof d.launch.args === 'string' ? d.launch.args : null,
    }
}

function mergeAliasesIntoResults(results) {
    const aliases = aliasesByAppId.value || {}
    if (!aliases || Object.keys(aliases).length === 0) return results
    return results.map((r) => {
        if (!r || r.kind !== 'app') return r
        if (r.aliases && r.aliases.length) return r
        const userAliases = aliases[r.id]
        if (userAliases && userAliases.length) {
            return { ...r, aliases: userAliases }
        }
        return r
    })
}

function mergeAliasesIntoApps(list) {
    const aliases = aliasesByAppId.value || {}
    if (!aliases || Object.keys(aliases).length === 0) return list
    return list.map((a) => {
        if (a.aliases && a.aliases.length) return a
        const userAliases = aliases[a.id] || aliases[a.exec]
        if (userAliases && userAliases.length) {
            return { ...a, aliases: userAliases }
        }
        return a
    })
}

function installSharedQueryWatcher() {
    if (queryWatcherInstalled) return
    queryWatcherInstalled = true
    watch(query, (newVal) => {
        if (searchDebounce) clearTimeout(searchDebounce)
        searchDebounce = setTimeout(() => runSharedSearch(newVal), SEARCH_DEBOUNCE_MS)
    })
    watch(kindFilter, () => {
        if (searchDebounce) clearTimeout(searchDebounce)
        if (query.value) {
            searchDebounce = setTimeout(() => runSharedSearch(query.value), SEARCH_DEBOUNCE_MS)
        }
    })
}

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
                const windowScale = 0.2
                width = Math.max(BASE_WIDTH, Math.floor(screenWidth * windowScale))

            } else {
                const webScreenWidth = window.screen.width
                if (webScreenWidth) {
                    const windowScale = 0.2
                    width = Math.max(BASE_WIDTH, Math.floor(webScreenWidth * windowScale))
                }
            }

            if (uiState.value === 'chatting') {
                height = CHAT_HEIGHT
            } else if (uiState.value === 'translating') {
                height = EXPANDED_HEIGHT
                width = Math.max(1000, Math.floor(width * 1.4))
            } else if (uiState.value === 'searching') {
                height = EXPANDED_HEIGHT
                if (query.value && query.value.trim().toLowerCase().startsWith('ff ')) {
                    width = Math.max(1000, Math.floor(width * 1.4))
                }
            } else if (uiState.value === 'executing') {
                height = EXPANDED_HEIGHT
            }

            await appWindow.setSize(new LogicalSize(width, height))

            if (searchInput.value) searchInput.value.focus()

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

        updateWindowSize()
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
            await loadTopFrecency()
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

    async function loadTopFrecency() {
        try {
            const top = await invoke('get_top_frecency', {
                limit: FREQUENT_APP_LIMIT,
                kindFilter: 'app',
            })
            topFrecencyApps.value = (top || [])
                .filter((a) => a && a.kind === 'app' && a.content)
                .map((a) => ({
                    exec: a.content,
                    name: a.name,
                    icon: a.icon || null,
                    frequency: a.frequency,
                    lastAccessed: a.last_accessed,
                }))
        } catch (e) {
            console.error('Failed to load top frecency', e)
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
                action.icon = item.icon;
            } else if (item.alias) { // Script
                action.id = 'script:' + item.alias;
                action.kind = 'script';
                action.content = item.path;
                action.name = item.alias;
            } else if (item.address) {
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
                return;
            }

            await invoke('record_action', { action })
            loadRecentActions()
            loadTopFrecency()
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
        return discoverResults.value
            .filter((d) => d && d.kind === 'app')
            .map((d) => discoverableToApp(d))
            .filter(Boolean)
            .slice(0, 10)
    })

    const topApps = computed(() => {
        if (query.value) return []
        const execToApp = new Map(apps.value.map((a) => [a.exec, a]))
        const merged = mergeAliasesIntoApps(apps.value)
        const seen = new Set()
        const items = []
        for (const tf of topFrecencyApps.value) {
            const app = execToApp.get(tf.exec)
            if (!app) continue
            if (seen.has(app.id)) continue
            seen.add(app.id)
            const enriched = merged.find((a) => a.id === app.id) || app
            items.push({ ...enriched, score: null })
            if (items.length >= FREQUENT_APP_LIMIT) break
        }
        return items
    })

    const filteredScripts = computed(() => {
        if (!query.value) return scripts.value
        const q = query.value.toLowerCase()

        const fromDiscover = discoverResults.value
            .filter((d) => d && d.kind === 'script')
            .map((d) => discoverableToScript(d))
            .filter(Boolean)
        if (fromDiscover.length > 0) return fromDiscover

        let matches = scripts.value.filter(s => s.alias.toLowerCase().includes(q))

        const recentMap = new Map()
        recentActions.value.forEach((action, index) => {
            if (action.kind === 'script') {
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

const filteredFiles = computed(() => {
        if (!query.value) return []
        return discoverResults.value
            .filter((d) => d && (d.kind === 'recent_file' || d.kind === 'shortcut'))
            .map((d) => d.launch && typeof d.launch.path === 'string' ? d.launch.path : '')
            .filter((p) => !!p)
    })

    // Watchers
    installSharedQueryWatcher()

    watch(query, (newVal) => {
        if (matchedTool.value) {
            selectedIndex.value = 0
        } else {
            const hasWindows = windows.value.some(w =>
                w.title.toLowerCase().includes(newVal.toLowerCase()) ||
                w.class.toLowerCase().includes(newVal.toLowerCase())
            );
            const hasApps = discoverResults.value.some((d) => d && d.kind === 'app')
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
        } else if (!newVal && (uiState.value === 'searching' || uiState.value === 'translating')) {
            uiState.value = 'idle'
            updateWindowSize()
        }

        if (newVal && newVal.startsWith('tr ')) {
            if (uiState.value !== 'translating') {
                uiState.value = 'translating'
                updateWindowSize()
            }
        } else if (uiState.value === 'translating') {
            uiState.value = 'searching'
            updateWindowSize()
        }

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
                    const includeHidden = config.value?.file_search?.include_hidden || false
                    files.value = await invoke('search_files', {
                        query: fileQuery,
                        path: home,
                        includeHidden
                    })
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

    async function subscribeAppUpdates() {
        const { listen } = await import('@tauri-apps/api/event')
        const offApps = await listen('apps-updated', (e) => {
            apps.value = e.payload
            loadTopFrecency()
        })
        const offAliases = await listen('aliases-updated', (e) => {
            const payload = e.payload || {}
            if (payload.app_id && Array.isArray(payload.aliases)) {
                aliasesByAppId.value = {
                    ...aliasesByAppId.value,
                    [payload.app_id]: payload.aliases,
                }
            }
        })
        return () => {
            offApps()
            offAliases()
        }
    }

    async function setAlias(appId, aliases) {
        try {
            await invoke('set_alias', { appId, aliases })
        } catch (e) {
            console.error('Failed to set alias', e)
            throw e
        }
    }

    async function removeAlias(appId, alias) {
        try {
            await invoke('remove_alias', { appId, alias })
        } catch (e) {
            console.error('Failed to remove alias', e)
            throw e
        }
    }

    async function listAliases() {
        try {
            const list = await invoke('list_aliases')
            const next = {}
            for (const entry of list || []) {
                if (entry && entry.app_id) {
                    next[entry.app_id] = entry.aliases || []
                }
            }
            aliasesByAppId.value = next
            return next
        } catch (e) {
            console.error('Failed to list aliases', e)
            return {}
        }
    }

    async function listCategories() {
        try {
            return await invoke('list_categories')
        } catch (e) {
            console.error('Failed to list categories', e)
            return []
        }
    }

    function setKindFilter(kindId) {
        const known = KIND_FILTER_OPTIONS.some((o) => o.id === kindId)
        if (!known) return
        if (kindFilter.value === kindId) return
        kindFilter.value = kindId
        selectedIndex.value = 0
    }

    function isKindFilterActive(kindId) {
        return kindFilter.value === kindId
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
        kindFilter,

        // Computed
        matchedTool,
        filteredWindows,
        filteredApps,
        filteredScripts,
        filteredFiles,
        topApps,
        scoredApps,
        discoverResults,
        aliasesByAppId,

        // Actions
        updateWindowSize,
        hideWindow,
        focusWindow,
        reloadConfig,
        loadData,
        recordAction,
        clearActions,
        subscribeAppUpdates,
        loadTopFrecency,
        setAlias,
        removeAlias,
        listAliases,
        listCategories,
        setKindFilter,
        isKindFilterActive,
        recentActions,
        KIND_FILTER_OPTIONS,
    }
}

export function _resetOmnibarForTests() {
    uiState.value = 'idle'
    query.value = ''
    config.value = null
    apps.value = []
    windows.value = []
    files.value = []
    scripts.value = []
    recentActions.value = []
    selectedIndex.value = 0
    showSettings.value = false
    searchInput.value = null
    scoredApps.value = []
    topFrecencyApps.value = []
    aliasesByAppId.value = {}
    kindFilter.value = 'all'
    discoverResults.value = []
    if (searchDebounce) {
        clearTimeout(searchDebounce)
        searchDebounce = null
    }
    lastSearchToken++
}

export const _internals = {
    SEARCH_DEBOUNCE_MS,
    FREQUENT_APP_LIMIT,
}