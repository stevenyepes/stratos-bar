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

    it('exposes search_apps invocation when query becomes non-empty', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'search_apps') {
                return Promise.resolve([
                    { app: makeApp('chrome', 'Chrome', 'chrome', ['Network']), score: 2.5, matched_field: 'name_prefix' },
                    { app: makeApp('chromium', 'Chromium', 'chromium', ['Network']), score: 1.8, matched_field: 'name_prefix' },
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

        expect(invokeMock).toHaveBeenCalledWith('search_apps', { query: 'chr', limit: 25 })
        expect(ctx.scoredApps.value.length).toBe(2)
        expect(ctx.filteredApps.value[0].name).toBe('Chrome')
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
        expect(invokeMock).not.toHaveBeenCalledWith('search_apps', expect.any(Object))
        await vi.advanceTimersByTimeAsync(200)
        const calls = invokeMock.mock.calls.filter((c) => c[0] === 'search_apps')
        expect(calls.length).toBe(1)
        expect(calls[0][1]).toEqual({ query: 'chr', limit: 25 })
        delete process.env.DEBUG_OMNIBAR
    })

    it('clears scoredApps when query is emptied', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'search_apps') {
                return Promise.resolve([
                    { app: makeApp('chrome', 'Chrome', 'chrome'), score: 1.5, matched_field: 'name_prefix' },
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
            if (cmd === 'search_apps') {
                return Promise.resolve([
                    { app: makeApp('chrome', 'Chrome', 'chrome'), score: 1.5, matched_field: 'name_prefix' },
                ])
            }
            return Promise.resolve([])
        })

        const ctx = useOmnibar()
        ctx.aliasesByAppId.value = { chrome: ['browser'] }
        ctx.query.value = 'chrome'
        await new Promise((r) => setTimeout(r, 150))
        await nextTick()
        expect(ctx.scoredApps.value[0].app.aliases).toEqual(['browser'])
    })

    it('does not clobber existing aliases on app entries', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'search_apps') {
                return Promise.resolve([
                    { app: { ...makeApp('chrome', 'Chrome', 'chrome'), aliases: ['builtin'] }, score: 1.5, matched_field: 'name_prefix' },
                ])
            }
            return Promise.resolve([])
        })

        const ctx = useOmnibar()
        ctx.aliasesByAppId.value = { chrome: ['browser'] }
        ctx.query.value = 'chrome'
        await new Promise((r) => setTimeout(r, 150))
        await nextTick()
        expect(ctx.scoredApps.value[0].app.aliases).toEqual(['builtin'])
    })
})