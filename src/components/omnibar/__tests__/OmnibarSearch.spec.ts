import { mount, flushPromises } from '@vue/test-utils'
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