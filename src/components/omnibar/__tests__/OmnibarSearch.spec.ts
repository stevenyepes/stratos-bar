import { mount, flushPromises } from '@vue/test-utils'
import { nextTick } from 'vue'
import { describe, it, expect, beforeEach, vi } from 'vitest'
import OmnibarSearch from '../OmnibarSearch.vue'
import { createVuetify } from 'vuetify'
import * as components from 'vuetify/components'
import * as directives from 'vuetify/directives'
import { _resetOmnibarForTests } from '../../../composables/useOmnibar'

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

vi.mock('../../theme', () => ({
    applyTheme: () => {},
    themePresets: [],
}))

vi.mock('../../skills', () => ({
    default: { match: () => null },
}))

vi.mock('vuetify', async () => {
    const actual = await vi.importActual('vuetify')
    return {
        ...actual,
        useTheme: () => ({ themes: { value: { dark: { colors: {} } } } }),
    }
})

const vuetify = createVuetify({ components, directives })

function fireKey(wrapper, key, ctrl = false) {
    const input = wrapper.find('input.search-input')
    input.trigger('keydown', {
        key,
        ctrlKey: ctrl,
        preventDefault: () => {},
    })
}

describe('OmnibarSearch', () => {
    let wrapper

    beforeEach(() => {
        invokeMock.mockReset()
        listenMock.mockReset()
        listenMock.mockImplementation(() => Promise.resolve(() => {}))
        invokeMock.mockImplementation(() => Promise.resolve([]))
        _resetOmnibarForTests()
        wrapper = mount(OmnibarSearch, {
            global: { plugins: [vuetify] },
            attachTo: document.body,
        })
    })

    it('renders an empty-state settings item when no query is entered', () => {
        expect(wrapper.text()).toContain('Settings')
    })

    it('shows the SUGGESTED header in idle state', () => {
        const header = wrapper.findAll('.section-header').map((h) => h.text())
        expect(header.some((t) => t.includes('SUGGESTED'))).toBe(true)
    })

    it('renders nothing in TOP APPS section when frecency list is empty', () => {
        const html = wrapper.html()
        expect(html).not.toContain('TOP APPS')
    })

    it('does not show kind filter chips while query is empty', () => {
        const row = wrapper.find('[data-testid="kind-filter-row"]')
        expect(row.exists()).toBe(false)
    })
})

describe('OmnibarSearch – kind filter chips', () => {
    let wrapper

    beforeEach(() => {
        invokeMock.mockReset()
        listenMock.mockReset()
        listenMock.mockImplementation(() => Promise.resolve(() => {}))
        invokeMock.mockImplementation(() => Promise.resolve([]))
        _resetOmnibarForTests()
        wrapper = mount(OmnibarSearch, {
            global: { plugins: [vuetify] },
            attachTo: document.body,
        })
    })

    async function setQuery(value) {
        const input = wrapper.find('input.search-input')
        input.element.value = value
        await input.trigger('input')
        await input.trigger('change')
    }

    it('renders five chips labelled All / Apps / Scripts / Files / Recent when a query is entered', async () => {
        await setQuery('chrome')
        await flushPromises()
        const row = wrapper.find('[data-testid="kind-filter-row"]')
        expect(row.exists()).toBe(true)
        const chips = wrapper.findAll('[data-testid^="kind-chip-"]')
        expect(chips.length).toBe(5)
        const labels = chips.map((c) => c.text())
        expect(labels.some((t) => t.includes('All'))).toBe(true)
        expect(labels.some((t) => t.includes('Apps'))).toBe(true)
        expect(labels.some((t) => t.includes('Scripts'))).toBe(true)
        expect(labels.some((t) => t.includes('Files'))).toBe(true)
        expect(labels.some((t) => t.includes('Recent'))).toBe(true)
    })

    it('marks the All chip as active by default', async () => {
        await setQuery('chrome')
        await flushPromises()
        const allChip = wrapper.find('[data-testid="kind-chip-all"]')
        expect(allChip.classes()).toContain('kind-chip-active')
        const appsChip = wrapper.find('[data-testid="kind-chip-apps"]')
        expect(appsChip.classes()).not.toContain('kind-chip-active')
    })

    it('clicking the Apps chip toggles its active state', async () => {
        await setQuery('chrome')
        await flushPromises()
        const appsChip = wrapper.find('[data-testid="kind-chip-apps"]')
        await appsChip.trigger('click')
        await flushPromises()
        expect(appsChip.classes()).toContain('kind-chip-active')
        const allChip = wrapper.find('[data-testid="kind-chip-all"]')
        expect(allChip.classes()).not.toContain('kind-chip-active')
    })

    it('clicking each chip in turn activates only that chip', async () => {
        await setQuery('chrome')
        await flushPromises()
        const ids = ['apps', 'scripts', 'files', 'recent']
        for (const id of ids) {
            const chip = wrapper.find(`[data-testid="kind-chip-${id}"]`)
            await chip.trigger('click')
            await flushPromises()
            expect(chip.classes()).toContain('kind-chip-active')
            for (const other of ['all', 'apps', 'scripts', 'files', 'recent']) {
                if (other === id) continue
                const otherChip = wrapper.find(`[data-testid="kind-chip-${other}"]`)
                expect(otherChip.classes()).not.toContain('kind-chip-active')
            }
        }
    })

    it('Ctrl+1 keyboard shortcut activates the Apps filter', async () => {
        await setQuery('chrome')
        await flushPromises()
        fireKey(wrapper, '1', true)
        await flushPromises()
        const appsChip = wrapper.find('[data-testid="kind-chip-apps"]')
        expect(appsChip.classes()).toContain('kind-chip-active')
    })

    it('Ctrl+2 keyboard shortcut activates the Scripts filter', async () => {
        await setQuery('chrome')
        await flushPromises()
        fireKey(wrapper, '2', true)
        await flushPromises()
        const scriptsChip = wrapper.find('[data-testid="kind-chip-scripts"]')
        expect(scriptsChip.classes()).toContain('kind-chip-active')
    })

    it('Ctrl+3 keyboard shortcut activates the Files filter', async () => {
        await setQuery('chrome')
        await flushPromises()
        fireKey(wrapper, '3', true)
        await flushPromises()
        const filesChip = wrapper.find('[data-testid="kind-chip-files"]')
        expect(filesChip.classes()).toContain('kind-chip-active')
    })

    it('Ctrl+4 keyboard shortcut activates the Recent filter', async () => {
        await setQuery('chrome')
        await flushPromises()
        fireKey(wrapper, '4', true)
        await flushPromises()
        const recentChip = wrapper.find('[data-testid="kind-chip-recent"]')
        expect(recentChip.classes()).toContain('kind-chip-active')
    })

    it('Ctrl+0 keyboard shortcut activates the All filter', async () => {
        await setQuery('chrome')
        await flushPromises()
        fireKey(wrapper, '1', true)
        await flushPromises()
        fireKey(wrapper, '0', true)
        await flushPromises()
        const allChip = wrapper.find('[data-testid="kind-chip-all"]')
        expect(allChip.classes()).toContain('kind-chip-active')
        const appsChip = wrapper.find('[data-testid="kind-chip-apps"]')
        expect(appsChip.classes()).not.toContain('kind-chip-active')
    })

    it('dispatches search_discoverable with the correct kinds payload after a chip click', async () => {
        await setQuery('chr')
        await new Promise((r) => setTimeout(r, 150))
        await flushPromises()
        invokeMock.mockClear()

        const scriptsChip = wrapper.find('[data-testid="kind-chip-scripts"]')
        await scriptsChip.trigger('click')
        await new Promise((r) => setTimeout(r, 150))
        await flushPromises()

        const calls = invokeMock.mock.calls.filter((c) => c[0] === 'search_discoverable')
        expect(calls.length).toBeGreaterThan(0)
        const lastCall = calls[calls.length - 1]
        expect(lastCall[1]).toEqual(expect.objectContaining({ query: 'chr', limit: 25, kinds: ['script'] }))
    })

    it('dispatches search_discoverable without kinds when All is active', async () => {
        await setQuery('chr')
        await new Promise((r) => setTimeout(r, 150))
        await flushPromises()
        const calls = invokeMock.mock.calls.filter((c) => c[0] === 'search_discoverable')
        expect(calls.length).toBeGreaterThan(0)
        const lastCall = calls[calls.length - 1]
        expect(lastCall[1]).toEqual({ query: 'chr', limit: 25 })
        expect(lastCall[1]).not.toHaveProperty('kinds')
    })
})

describe('OmnibarSearch – TopAppsRail wiring', () => {
    let wrapper

    beforeEach(() => {
        invokeMock.mockReset()
        listenMock.mockReset()
        listenMock.mockImplementation(() => Promise.resolve(() => {}))
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'list_apps') {
                return Promise.resolve([
                    { id: 'chrome', name: 'Chrome', exec: 'chrome', icon: null, source: 'desktop', categories: ['Network'] },
                    { id: 'code', name: 'Code', exec: 'code', icon: null, source: 'desktop', categories: ['Development'] },
                    { id: 'figma', name: 'Figma', exec: 'figma', icon: null, source: 'desktop', categories: ['Graphics'] },
                ])
            }
            if (cmd === 'get_top_frecency') {
                return Promise.resolve([
                    { id: 'app:chrome', kind: 'app', name: 'Chrome', content: 'chrome', icon: null, frequency: 50, last_accessed: Date.now() },
                    { id: 'app:code', kind: 'app', name: 'Code', content: 'code', icon: null, frequency: 30, last_accessed: Date.now() },
                ])
            }
            return Promise.resolve([])
        })
        _resetOmnibarForTests()
    })

    it('mounts <TopAppsRail> with the entries returned by useOmnibar.topApps', async () => {
        const { useOmnibar } = await import('../../../composables/useOmnibar')
        const ctx = useOmnibar()
        await ctx.loadData()
        await nextTick()
        await flushPromises()
        wrapper = mount(OmnibarSearch, {
            global: { plugins: [vuetify] },
            attachTo: document.body,
        })
        await flushPromises()
        const rail = wrapper.find('[data-testid="top-apps-rail"]')
        expect(rail.exists()).toBe(true)
        const tiles = wrapper.findAll('[data-testid^="top-app-"]')
        expect(tiles.length).toBeGreaterThan(0)
    })

    it('routes TopAppsRail launch events through launch_app invoke', async () => {
        const { useOmnibar } = await import('../../../composables/useOmnibar')
        const ctx = useOmnibar()
        await ctx.loadData()
        await nextTick()
        await flushPromises()
        wrapper = mount(OmnibarSearch, {
            global: { plugins: [vuetify] },
            attachTo: document.body,
        })
        await flushPromises()
        invokeMock.mockClear()
        await wrapper.find('[data-testid="top-app-chrome"]').trigger('click')
        await flushPromises()
        const launchCalls = invokeMock.mock.calls.filter((c) => c[0] === 'launch_app')
        expect(launchCalls.length).toBe(1)
        expect(launchCalls[0][1]).toEqual(expect.objectContaining({ execCmd: 'chrome' }))
    })
})

describe('OmnibarSearch – CategoryFilterBar wiring', () => {
    let wrapper

    beforeEach(() => {
        invokeMock.mockReset()
        listenMock.mockReset()
        listenMock.mockImplementation(() => Promise.resolve(() => {}))
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'list_apps') {
                return Promise.resolve([
                    { id: 'chrome', name: 'Chrome', exec: 'chrome', icon: null, source: 'desktop', categories: ['Network'] },
                    { id: 'code', name: 'Code', exec: 'code', icon: null, source: 'desktop', categories: ['Development'] },
                    { id: 'figma', name: 'Figma', exec: 'figma', icon: null, source: 'desktop', categories: ['Graphics'] },
                ])
            }
            if (cmd === 'search_discoverable') {
                return Promise.resolve([
                    { kind: 'app', id: 'code', name: 'Code', score: 1.5, source: 'app:desktop', launch: { type: 'app', exec: 'code' }, category: 'Development', aliases: [] },
                    { kind: 'app', id: 'chrome', name: 'Chrome', score: 1.0, source: 'app:desktop', launch: { type: 'app', exec: 'chrome' }, category: 'Network', aliases: [] },
                ])
            }
            return Promise.resolve([])
        })
        _resetOmnibarForTests()
        wrapper = mount(OmnibarSearch, {
            global: { plugins: [vuetify] },
            attachTo: document.body,
        })
    })

    async function setQuery(value) {
        const input = wrapper.find('input.search-input')
        input.element.value = value
        await input.trigger('input')
        await input.trigger('change')
    }

    it('renders <CategoryFilterBar> chips for each app category', async () => {
        const { useOmnibar } = await import('../../../composables/useOmnibar')
        const ctx = useOmnibar()
        await ctx.loadData()
        await setQuery('c')
        await new Promise((r) => setTimeout(r, 150))
        await flushPromises()
        const bar = wrapper.find('[data-testid="category-filter-bar"]')
        expect(bar.exists()).toBe(true)
        expect(bar.find('[data-testid="category-chip-all"]').exists()).toBe(true)
        expect(bar.find('[data-testid="category-chip-development"]').exists()).toBe(true)
        expect(bar.find('[data-testid="category-chip-network"]').exists()).toBe(true)
        expect(bar.find('[data-testid="category-chip-graphics"]').exists()).toBe(true)
    })

    it('clicking a category chip restricts visible applications to that category', async () => {
        const { useOmnibar } = await import('../../../composables/useOmnibar')
        const ctx = useOmnibar()
        await ctx.loadData()
        await setQuery('c')
        await new Promise((r) => setTimeout(r, 150))
        await flushPromises()
        await wrapper.find('[data-testid="category-chip-development"]').trigger('click')
        await flushPromises()
        const headers = wrapper.findAll('.section-header').map((h) => h.text())
        expect(headers).toContain('APPLICATIONS')
        expect(wrapper.text()).toContain('Code')
        expect(wrapper.text()).not.toContain('Chrome')
    })

    it('clicking the same chip again clears the category filter', async () => {
        const { useOmnibar } = await import('../../../composables/useOmnibar')
        const ctx = useOmnibar()
        await ctx.loadData()
        await setQuery('c')
        await new Promise((r) => setTimeout(r, 150))
        await flushPromises()
        const devChip = wrapper.find('[data-testid="category-chip-development"]')
        await devChip.trigger('click')
        await flushPromises()
        await devChip.trigger('click')
        await flushPromises()
        const allChip = wrapper.find('[data-testid="category-chip-all"]')
        expect(allChip.classes()).toContain('category-chip-active')
        expect(wrapper.text()).toContain('Chrome')
    })
})

describe('OmnibarSearch – EmptyIndex LaunchError surfaces to UI', () => {
    let wrapper
    let errorSpy

    beforeEach(() => {
        invokeMock.mockReset()
        listenMock.mockReset()
        listenMock.mockImplementation(() => Promise.resolve(() => {}))
        invokeMock.mockImplementation(() => Promise.resolve([]))
        _resetOmnibarForTests()
        errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
    })

    afterEach(() => {
        errorSpy.mockRestore()
    })

    it('surfaces LaunchError::UnvalidatedExec text in the footer when launch_app rejects with an empty index', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'list_apps') {
                return Promise.resolve([
                    { id: 'chrome', name: 'Chrome', exec: 'chrome', icon: null, source: 'desktop', categories: ['Network'] },
                ])
            }
            if (cmd === 'get_top_frecency') {
                return Promise.resolve([
                    { id: 'app:chrome', kind: 'app', name: 'Chrome', content: 'chrome', icon: null, frequency: 5, last_accessed: Date.now() },
                ])
            }
            if (cmd === 'launch_app') {
                return Promise.reject('exec command was not validated against the canonical index entry')
            }
            return Promise.resolve([])
        })
        const { useOmnibar } = await import('../../../composables/useOmnibar')
        const ctx = useOmnibar()
        await ctx.loadData()
        await nextTick()
        await flushPromises()
        wrapper = mount(OmnibarSearch, {
            global: { plugins: [vuetify] },
            attachTo: document.body,
        })
        await flushPromises()
        await wrapper.find('[data-testid="top-app-chrome"]').trigger('click')
        await flushPromises()
        const errorBanner = wrapper.find('[data-testid="launch-error"]')
        expect(errorBanner.exists()).toBe(true)
        expect(errorBanner.text()).toMatch(/validated/i)
        const launchCalls = invokeMock.mock.calls.filter((c) => c[0] === 'launch_app')
        expect(launchCalls.length).toBe(1)
    })

    it('clears the launch error footer element after a successful launch', async () => {
        let attempts = 0
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'list_apps') {
                return Promise.resolve([
                    { id: 'chrome', name: 'Chrome', exec: 'chrome', icon: null, source: 'desktop', categories: ['Network'] },
                ])
            }
            if (cmd === 'get_top_frecency') {
                return Promise.resolve([
                    { id: 'app:chrome', kind: 'app', name: 'Chrome', content: 'chrome', icon: null, frequency: 5, last_accessed: Date.now() },
                ])
            }
            if (cmd === 'launch_app') {
                attempts += 1
                if (attempts === 1) return Promise.reject('exec command was not validated against the canonical index entry')
                return Promise.resolve()
            }
            return Promise.resolve([])
        })
        const { useOmnibar } = await import('../../../composables/useOmnibar')
        const ctx = useOmnibar()
        await ctx.loadData()
        await nextTick()
        await flushPromises()
        wrapper = mount(OmnibarSearch, {
            global: { plugins: [vuetify] },
            attachTo: document.body,
        })
        await flushPromises()
        await wrapper.find('[data-testid="top-app-chrome"]').trigger('click')
        await flushPromises()
        expect(wrapper.find('[data-testid="launch-error"]').exists()).toBe(true)
        await wrapper.find('[data-testid="top-app-chrome"]').trigger('click')
        await flushPromises()
        expect(wrapper.find('[data-testid="launch-error"]').exists()).toBe(false)
    })
})

describe('OmnibarSearch – aliases-updated event subscription through useOmnibar', () => {
    let wrapper
    let listeners

    beforeEach(() => {
        invokeMock.mockReset()
        listenMock.mockReset()
        listeners = {}
        listenMock.mockImplementation((event, handler) => {
            listeners[event] = handler
            return Promise.resolve(() => {})
        })
        invokeMock.mockImplementation(() => Promise.resolve([]))
        _resetOmnibarForTests()
        wrapper = mount(OmnibarSearch, {
            global: { plugins: [vuetify] },
            attachTo: document.body,
        })
    })

    it('mounts a listener for aliases-updated alongside the existing apps-updated subscription', async () => {
        const { useOmnibar } = await import('../../../composables/useOmnibar')
        const ctx = useOmnibar()
        await ctx.subscribeAppUpdates()
        expect(typeof listeners['apps-updated']).toBe('function')
        expect(typeof listeners['aliases-updated']).toBe('function')
    })

    it('aliases-updated event payloads land in aliasesByAppId via the omnibar singleton', async () => {
        const { useOmnibar } = await import('../../../composables/useOmnibar')
        const ctx = useOmnibar()
        await ctx.subscribeAppUpdates()
        await listeners['aliases-updated']({
            payload: { app_id: 'chrome', aliases: ['browser', 'web'] },
        })
        expect(ctx.aliasesByAppId.value['chrome']).toEqual(['browser', 'web'])
    })
})