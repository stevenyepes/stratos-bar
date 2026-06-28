import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'

const invokeMock = vi.fn()
const listenMock = vi.fn(() => Promise.resolve(() => {}))

vi.mock('@tauri-apps/api/core', () => ({
    invoke: (...args) => invokeMock(...args),
    convertFileSrc: (s) => s,
}))

vi.mock('@tauri-apps/api/event', () => ({
    listen: (...args) => listenMock(...args),
}))

vi.mock('@tauri-apps/api/window', () => ({
    getCurrentWindow: () => ({
        setSize: () => Promise.resolve(),
        setFocus: () => Promise.resolve(),
        hide: () => Promise.resolve(),
        show: () => Promise.resolve(),
    }),
    currentMonitor: () => Promise.resolve(null),
}))

vi.mock('@tauri-apps/api/dpi', () => ({
    LogicalSize: class { constructor(w, h) { this.width = w; this.height = h } },
}))

vi.mock('@tauri-apps/api/path', () => ({
    homeDir: () => Promise.resolve('/home/test'),
}))

vi.mock('../theme', () => ({
    applyTheme: () => {},
    themePresets: [],
}))

vi.mock('../../skills', () => ({
    default: { match: () => null },
}))

vi.mock('vuetify', () => ({
    useTheme: () => ({ themes: { value: { dark: { colors: {} } } } }),
}))

import { useOmnibar, _resetOmnibarForTests } from '../useOmnibar'
import { nextTick } from 'vue'

function makeApp(id, name, exec, categories = [], source = 'desktop', aliases = []) {
    return {
        id,
        name,
        exec,
        generic_name: null,
        description: null,
        keywords: [],
        try_exec: null,
        icon: null,
        categories,
        startup_wm_class: null,
        source,
        path: `/usr/share/applications/${id}.desktop`,
        aliases,
    }
}

function makeDiscoverable(overrides = {}) {
    return {
        kind: 'app',
        id: 'chrome',
        name: 'Chrome',
        description: null,
        icon: null,
        keywords: [],
        score: 1.5,
        source: 'app:desktop',
        launch: { type: 'app', exec: 'chrome' },
        category: null,
        aliases: [],
        ...overrides,
    }
}

describe('useOmnibar – search and frecency', () => {
    beforeEach(() => {
        invokeMock.mockReset()
        listenMock.mockReset()
        listenMock.mockImplementation(() => Promise.resolve(() => {}))
        invokeMock.mockImplementation(() => Promise.resolve([]))
        vi.useFakeTimers()
        _resetOmnibarForTests()
    })

    afterEach(() => {
        vi.useRealTimers()
    })

    it('exposes search_discoverable invocation when query becomes non-empty', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'search_discoverable') {
                return Promise.resolve([
                    makeDiscoverable({ id: 'chrome', name: 'Chrome', score: 2.5 }),
                    makeDiscoverable({ id: 'chromium', name: 'Chromium', score: 1.8, launch: { type: 'app', exec: 'chromium' } }),
                ])
            }
            return Promise.resolve([])
        })

        const ctx = useOmnibar()
        ctx.apps.value = [
            makeApp('chrome', 'Chrome', 'chrome', ['Network']),
            makeApp('chromium', 'Chromium', 'chromium', ['Network']),
        ]
        ctx.query.value = 'chr'
        await vi.advanceTimersByTimeAsync(150)
        await nextTick()

        expect(invokeMock).toHaveBeenCalledWith('search_discoverable', { query: 'chr', limit: 25 })
        expect(ctx.scoredApps.value.length).toBe(2)
        expect(ctx.filteredApps.value[0].name).toBe('Chrome')
        expect(ctx.filteredApps.value[0].exec).toBe('chrome')
        expect(ctx.filteredApps.value[1].name).toBe('Chromium')
    })

    it('debounces search invocations when query changes rapidly', async () => {
        process.env.DEBUG_OMNIBAR = '1'
        invokeMock.mockImplementation(() => Promise.resolve([]))
        const ctx = useOmnibar()
        ctx.query.value = 'c'
        ctx.query.value = 'ch'
        ctx.query.value = 'chr'
        await vi.advanceTimersByTimeAsync(50)
        expect(invokeMock).not.toHaveBeenCalledWith('search_discoverable', expect.any(Object))
        await vi.advanceTimersByTimeAsync(200)
        const calls = invokeMock.mock.calls.filter((c) => c[0] === 'search_discoverable')
        expect(calls.length).toBe(1)
        expect(calls[0][1]).toEqual({ query: 'chr', limit: 25 })
        delete process.env.DEBUG_OMNIBAR
    })

    it('clears discoverResults when query is emptied', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'search_discoverable') {
                return Promise.resolve([
                    makeDiscoverable({ id: 'chrome', name: 'Chrome', score: 1.5 }),
                ])
            }
            return Promise.resolve([])
        })
        const ctx = useOmnibar()
        ctx.query.value = 'c'
        await vi.advanceTimersByTimeAsync(150)
        await nextTick()
        expect(ctx.scoredApps.value.length).toBe(1)
        ctx.query.value = ''
        await vi.advanceTimersByTimeAsync(150)
        await nextTick()
        expect(ctx.scoredApps.value).toEqual([])
        expect(ctx.filteredApps.value).toEqual([])
        expect(ctx.discoverResults.value).toEqual([])
    })

    it('builds topApps from frecency results when query is empty', async () => {
        const chrome = makeApp('chrome', 'Chrome', 'chrome', ['Network'])
        const code = makeApp('code', 'Code', 'code', ['Development'])
        const fig = makeApp('figma', 'Figma', 'figma', ['Graphics'])
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'list_apps') return Promise.resolve([chrome, code, fig])
            if (cmd === 'get_top_frecency') {
                return Promise.resolve([
                    { id: 'app:chrome', kind: 'app', name: 'Chrome', content: 'chrome', icon: null, frequency: 50, last_accessed: Date.now() },
                    { id: 'app:code', kind: 'app', name: 'Code', content: 'code', icon: null, frequency: 30, last_accessed: Date.now() },
                    { id: 'app:figma', kind: 'app', name: 'Figma', content: 'figma', icon: null, frequency: 20, last_accessed: Date.now() },
                ])
            }
            return Promise.resolve([])
        })

        const ctx = useOmnibar()
        await ctx.loadData()
        await nextTick()

        expect(ctx.topApps.value.length).toBe(3)
        expect(ctx.topApps.value[0].name).toBe('Chrome')
        expect(ctx.topApps.value[1].name).toBe('Code')
        expect(ctx.topApps.value[2].name).toBe('Figma')
    })

    it('hides topApps when a query is active', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'list_apps') return Promise.resolve([makeApp('chrome', 'Chrome', 'chrome')])
            if (cmd === 'get_top_frecency') {
                return Promise.resolve([
                    { id: 'app:chrome', kind: 'app', name: 'Chrome', content: 'chrome', icon: null, frequency: 5, last_accessed: Date.now() },
                ])
            }
            return Promise.resolve([])
        })

        const ctx = useOmnibar()
        await ctx.loadData()
        await nextTick()
        ctx.query.value = 'foo'
        await nextTick()
        expect(ctx.topApps.value).toEqual([])
    })

    it('stashes aliases-updated events into aliasesByAppId', async () => {
        const listeners = {}
        listenMock.mockImplementation((event, handler) => {
            listeners[event] = handler
            return Promise.resolve(() => {})
        })

        const ctx = useOmnibar()
        await ctx.subscribeAppUpdates()
        expect(listeners['apps-updated']).toBeDefined()
        expect(listeners['aliases-updated']).toBeDefined()

        await listeners['aliases-updated']({ payload: { app_id: 'chrome', aliases: ['browser', 'web'] } })
        expect(ctx.aliasesByAppId.value.chrome).toEqual(['browser', 'web'])
    })

    it('rewrites apps value on apps-updated event and reloads frecency', async () => {
        const listeners = {}
        listenMock.mockImplementation((event, handler) => {
            listeners[event] = handler
            return Promise.resolve(() => {})
        })
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'get_top_frecency') return Promise.resolve([])
            return Promise.resolve([])
        })

        const ctx = useOmnibar()
        await ctx.subscribeAppUpdates()
        const payload = [makeApp('new', 'New App', 'new')]
        await listeners['apps-updated']({ payload })
        expect(ctx.apps.value).toEqual(payload)
        expect(invokeMock).toHaveBeenCalledWith('get_top_frecency', { limit: 8, kindFilter: 'app' })
    })

    it('filters out non-app entries from frecency topApps', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'list_apps') return Promise.resolve([makeApp('chrome', 'Chrome', 'chrome')])
            if (cmd === 'get_top_frecency') {
                return Promise.resolve([
                    { id: 'app:chrome', kind: 'app', name: 'Chrome', content: 'chrome', icon: null, frequency: 5, last_accessed: Date.now() },
                    { id: 'script:deploy', kind: 'script', name: 'deploy', content: '/srv/deploy.sh', icon: null, frequency: 99, last_accessed: Date.now() },
                    { id: 'window:focus', kind: 'window', name: 'firefox', content: 'firefox', icon: null, frequency: 99, last_accessed: Date.now() },
                ])
            }
            return Promise.resolve([])
        })

        const ctx = useOmnibar()
        await ctx.loadData()
        await nextTick()
        expect(ctx.topApps.value.length).toBe(1)
        expect(ctx.topApps.value[0].name).toBe('Chrome')
    })

    it('propagates setAlias errors so callers can react', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'set_alias') return Promise.reject('Invalid alias')
            return Promise.resolve([])
        })
        const ctx = useOmnibar()
        await expect(ctx.setAlias('chrome', ['bad alias'])).rejects.toBe('Invalid alias')
    })

    it('listAliases stores returned entries on the reactive map', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'list_aliases') {
                return Promise.resolve([
                    { app_id: 'chrome', aliases: ['browser'] },
                    { app_id: 'code', aliases: ['editor', 'ide'] },
                ])
            }
            return Promise.resolve([])
        })
        const ctx = useOmnibar()
        const result = await ctx.listAliases()
        expect(result.chrome).toEqual(['browser'])
        expect(result.code).toEqual(['editor', 'ide'])
        expect(ctx.aliasesByAppId.value.chrome).toEqual(['browser'])
    })
})

describe('useOmnibar – alias boost merge into scored apps', () => {
    beforeEach(() => {
        invokeMock.mockReset()
        listenMock.mockReset()
        listenMock.mockImplementation(() => Promise.resolve(() => {}))
        invokeMock.mockImplementation(() => Promise.resolve([]))
        _resetOmnibarForTests()
    })

    it('injects store-provided aliases into search results', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'search_discoverable') {
                return Promise.resolve([
                    makeDiscoverable({ id: 'chrome', name: 'Chrome' }),
                ])
            }
            return Promise.resolve([])
        })

        const ctx = useOmnibar()
        ctx.aliasesByAppId.value = { chrome: ['browser'] }
        ctx.query.value = 'chrome'
        await new Promise((r) => setTimeout(r, 150))
        await nextTick()
        expect(ctx.discoverResults.value[0].aliases).toEqual(['browser'])
        expect(ctx.scoredApps.value[0].app.aliases).toEqual(['browser'])
    })

    it('does not clobber existing aliases on app entries', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'search_discoverable') {
                return Promise.resolve([
                    makeDiscoverable({ id: 'chrome', name: 'Chrome', aliases: ['builtin'] }),
                ])
            }
            return Promise.resolve([])
        })

        const ctx = useOmnibar()
        ctx.aliasesByAppId.value = { chrome: ['browser'] }
        ctx.query.value = 'chrome'
        await new Promise((r) => setTimeout(r, 150))
        await nextTick()
        expect(ctx.discoverResults.value[0].aliases).toEqual(['builtin'])
        expect(ctx.scoredApps.value[0].app.aliases).toEqual(['builtin'])
    })
})

describe('useOmnibar – kind filter chips', () => {
    beforeEach(() => {
        invokeMock.mockReset()
        listenMock.mockReset()
        listenMock.mockImplementation(() => Promise.resolve(() => {}))
        invokeMock.mockImplementation(() => Promise.resolve([]))
        vi.useFakeTimers()
        _resetOmnibarForTests()
    })

    afterEach(() => {
        vi.useRealTimers()
    })

    it('defaults kindFilter to "all" and omits kinds from the backend payload', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'search_discoverable') return Promise.resolve([])
            return Promise.resolve([])
        })

        const ctx = useOmnibar()
        ctx.query.value = 'chr'
        await vi.advanceTimersByTimeAsync(150)
        await nextTick()

        expect(ctx.kindFilter.value).toBe('all')
        expect(ctx.isKindFilterActive('all')).toBe(true)
        const searchCalls = invokeMock.mock.calls.filter((c) => c[0] === 'search_discoverable')
        expect(searchCalls.length).toBeGreaterThan(0)
        const lastCall = searchCalls[searchCalls.length - 1]
        expect(lastCall[1]).toEqual({ query: 'chr', limit: 25 })
        expect(lastCall[1]).not.toHaveProperty('kinds')
    })

    it('setKindFilter("apps") forwards ["app"] as kinds to search_discoverable', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'search_discoverable') return Promise.resolve([])
            return Promise.resolve([])
        })

        const ctx = useOmnibar()
        ctx.query.value = 'chr'
        await vi.advanceTimersByTimeAsync(150)
        await nextTick()
        invokeMock.mockClear()

        ctx.setKindFilter('apps')
        await vi.advanceTimersByTimeAsync(150)
        await nextTick()

        expect(ctx.kindFilter.value).toBe('apps')
        expect(ctx.isKindFilterActive('apps')).toBe(true)
        const searchCalls = invokeMock.mock.calls.filter((c) => c[0] === 'search_discoverable')
        expect(searchCalls.length).toBe(1)
        expect(searchCalls[0][1]).toEqual({ query: 'chr', limit: 25, kinds: ['app'] })
    })

    it('setKindFilter("scripts") forwards ["script"] as kinds to search_discoverable', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'search_discoverable') return Promise.resolve([])
            return Promise.resolve([])
        })

        const ctx = useOmnibar()
        ctx.query.value = 'deploy'
        await vi.advanceTimersByTimeAsync(150)
        await nextTick()
        invokeMock.mockClear()

        ctx.setKindFilter('scripts')
        await vi.advanceTimersByTimeAsync(150)
        await nextTick()

        expect(ctx.kindFilter.value).toBe('scripts')
        const searchCalls = invokeMock.mock.calls.filter((c) => c[0] === 'search_discoverable')
        expect(searchCalls.length).toBe(1)
        expect(searchCalls[0][1]).toEqual({ query: 'deploy', limit: 25, kinds: ['script'] })
    })

    it('setKindFilter("files") forwards ["recent_file"] as kinds to search_discoverable', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'search_discoverable') return Promise.resolve([])
            return Promise.resolve([])
        })

        const ctx = useOmnibar()
        ctx.query.value = 'note'
        await vi.advanceTimersByTimeAsync(150)
        await nextTick()
        invokeMock.mockClear()

        ctx.setKindFilter('files')
        await vi.advanceTimersByTimeAsync(150)
        await nextTick()

        expect(ctx.kindFilter.value).toBe('files')
        const searchCalls = invokeMock.mock.calls.filter((c) => c[0] === 'search_discoverable')
        expect(searchCalls.length).toBe(1)
        expect(searchCalls[0][1]).toEqual({ query: 'note', limit: 25, kinds: ['recent_file'] })
    })

    it('setKindFilter("recent") forwards ["shortcut"] as kinds to search_discoverable', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'search_discoverable') return Promise.resolve([])
            return Promise.resolve([])
        })

        const ctx = useOmnibar()
        ctx.query.value = 'quote'
        await vi.advanceTimersByTimeAsync(150)
        await nextTick()
        invokeMock.mockClear()

        ctx.setKindFilter('recent')
        await vi.advanceTimersByTimeAsync(150)
        await nextTick()

        expect(ctx.kindFilter.value).toBe('recent')
        const searchCalls = invokeMock.mock.calls.filter((c) => c[0] === 'search_discoverable')
        expect(searchCalls.length).toBe(1)
        expect(searchCalls[0][1]).toEqual({ query: 'quote', limit: 25, kinds: ['shortcut'] })
    })

    it('cycles through all 5 filter options via setKindFilter and updates state', () => {
        const ctx = useOmnibar()
        const ids = ['all', 'apps', 'scripts', 'files', 'recent']
        for (const id of ids) {
            ctx.setKindFilter(id)
            expect(ctx.kindFilter.value).toBe(id)
            expect(ctx.isKindFilterActive(id)).toBe(true)
            expect(ctx.isKindFilterActive('all')).toBe(id === 'all')
        }
    })

    it('setKindFilter resets selectedIndex to keep navigation in sync', () => {
        const ctx = useOmnibar()
        ctx.selectedIndex.value = 7
        ctx.setKindFilter('apps')
        expect(ctx.selectedIndex.value).toBe(0)
        ctx.setKindFilter('scripts')
        expect(ctx.selectedIndex.value).toBe(0)
    })

    it('setKindFilter rejects unknown identifiers', () => {
        const ctx = useOmnibar()
        ctx.selectedIndex.value = 3
        ctx.setKindFilter('totally-bogus')
        expect(ctx.kindFilter.value).toBe('all')
        expect(ctx.selectedIndex.value).toBe(3)
    })

    it('changing kind filter while a query is active re-invokes search_discoverable with new kinds', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'search_discoverable') return Promise.resolve([])
            return Promise.resolve([])
        })

        const ctx = useOmnibar()
        ctx.query.value = 'chr'
        await vi.advanceTimersByTimeAsync(150)
        await nextTick()
        const initialCalls = invokeMock.mock.calls.filter((c) => c[0] === 'search_discoverable').length

        ctx.setKindFilter('scripts')
        await vi.advanceTimersByTimeAsync(150)
        await nextTick()

        const finalCalls = invokeMock.mock.calls.filter((c) => c[0] === 'search_discoverable')
        expect(finalCalls.length).toBe(initialCalls + 1)
        expect(finalCalls[finalCalls.length - 1][1]).toEqual({ query: 'chr', limit: 25, kinds: ['script'] })
    })

    it('setKindFilter is a no-op when the same filter is selected again', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'search_discoverable') return Promise.resolve([])
            return Promise.resolve([])
        })

        const ctx = useOmnibar()
        ctx.query.value = 'chr'
        ctx.setKindFilter('apps')
        await vi.advanceTimersByTimeAsync(150)
        await nextTick()
        invokeMock.mockClear()

        ctx.setKindFilter('apps')
        await vi.advanceTimersByTimeAsync(150)
        await nextTick()
        const searchCalls = invokeMock.mock.calls.filter((c) => c[0] === 'search_discoverable')
        expect(searchCalls.length).toBe(0)
    })

    it('exposes KIND_FILTER_OPTIONS with All / Apps / Scripts / Files / Recent', () => {
        const ctx = useOmnibar()
        const ids = ctx.KIND_FILTER_OPTIONS.map((o) => o.id)
        expect(ids).toEqual(['all', 'apps', 'scripts', 'files', 'recent'])
        const labels = ctx.KIND_FILTER_OPTIONS.map((o) => o.label)
        expect(labels).toEqual(['All', 'Apps', 'Scripts', 'Files', 'Recent'])
    })
})

describe('useOmnibar – discoverable result partitioning', () => {
    beforeEach(() => {
        invokeMock.mockReset()
        listenMock.mockReset()
        listenMock.mockImplementation(() => Promise.resolve(() => {}))
        invokeMock.mockImplementation(() => Promise.resolve([]))
        vi.useFakeTimers()
        _resetOmnibarForTests()
    })

    afterEach(() => {
        vi.useRealTimers()
    })

    it('partitions mixed-kind results into filteredApps, filteredScripts, and filteredFiles', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'search_discoverable') {
                return Promise.resolve([
                    makeDiscoverable({ id: 'chrome', name: 'Chrome', kind: 'app', launch: { type: 'app', exec: 'chrome' } }),
                    makeDiscoverable({ id: 'firefox', name: 'Firefox', kind: 'app', launch: { type: 'app', exec: 'firefox' } }),
                    {
                        kind: 'script', id: 'deploy', name: 'deploy',
                        launch: { type: 'script', path: '/srv/deploy.sh', args: null },
                        score: 2.0, source: 'config:script', description: null, icon: null, keywords: [], category: null, aliases: [],
                    },
                    {
                        kind: 'recent_file', id: 'note', name: 'note.md',
                        launch: { type: 'recent_file', path: '/home/test/note.md' },
                        score: 1.0, source: 'history:file', description: null, icon: null, keywords: [], category: null, aliases: [],
                    },
                ])
            }
            return Promise.resolve([])
        })

        const ctx = useOmnibar()
        ctx.query.value = 'ch'
        await vi.advanceTimersByTimeAsync(150)
        await nextTick()

        expect(ctx.filteredApps.value.length).toBe(2)
        expect(ctx.filteredApps.value.map((a) => a.name)).toEqual(['Chrome', 'Firefox'])

        expect(ctx.filteredScripts.value.length).toBe(1)
        expect(ctx.filteredScripts.value[0].alias).toBe('deploy')
        expect(ctx.filteredScripts.value[0].path).toBe('/srv/deploy.sh')

        expect(ctx.filteredFiles.value.length).toBe(1)
        expect(ctx.filteredFiles.value[0]).toBe('/home/test/note.md')
    })

    it('collapses the unified list to a single kind when filter is applied', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'search_discoverable') {
                return Promise.resolve([
                    {
                        kind: 'script', id: 'deploy', name: 'deploy',
                        launch: { type: 'script', path: '/srv/deploy.sh', args: null },
                        score: 1.5, source: 'config:script', description: null, icon: null, keywords: [], category: null, aliases: [],
                    },
                ])
            }
            return Promise.resolve([])
        })

        const ctx = useOmnibar()
        ctx.setKindFilter('scripts')
        ctx.query.value = 'deploy'
        await vi.advanceTimersByTimeAsync(150)
        await nextTick()

        expect(ctx.filteredApps.value.length).toBe(0)
        expect(ctx.filteredScripts.value.length).toBe(1)
        expect(ctx.filteredScripts.value[0].alias).toBe('deploy')
    })

    it('source field is stripped of "app:" prefix for app results', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'search_discoverable') {
                return Promise.resolve([
                    makeDiscoverable({ id: 'chrome', name: 'Chrome', source: 'app:desktop', launch: { type: 'app', exec: 'chrome' } }),
                ])
            }
            return Promise.resolve([])
        })

        const ctx = useOmnibar()
        ctx.query.value = 'chrome'
        await vi.advanceTimersByTimeAsync(150)
        await nextTick()

        expect(ctx.filteredApps.value[0].source).toBe('desktop')
    })
})