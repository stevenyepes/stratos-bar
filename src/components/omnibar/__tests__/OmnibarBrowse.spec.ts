import { mount, flushPromises } from '@vue/test-utils'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { nextTick } from 'vue'

const invokeMock = vi.fn()

vi.mock('@tauri-apps/api/core', () => ({
    invoke: (...args) => invokeMock(...args),
    convertFileSrc: (s) => s,
}))

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn(() => Promise.resolve(() => {})),
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

import OmnibarBrowse from '../OmnibarBrowse.vue'
import { createVuetify } from 'vuetify'
import * as components from 'vuetify/components'
import * as directives from 'vuetify/directives'

const vuetify = createVuetify({ components, directives })

function makeItem(id, name, exec, category, source) {
    return {
        kind: 'app',
        id,
        name,
        description: null,
        icon: null,
        keywords: [],
        score: 0,
        source: `app:${source}`,
        launch: { type: 'app', exec },
        category: category || null,
        aliases: [],
    }
}

function makeSectionsPayload() {
    const items = [
        makeItem('chrome', 'Chrome', 'chrome', 'Network', 'flatpak'),
        makeItem('firefox', 'Firefox', 'firefox', 'Network', 'snap'),
        makeItem('code', 'Code', 'code', 'Development', 'appimage'),
        makeItem('figma', 'Figma', 'figma', 'Graphics', 'desktop'),
    ]
    const apps_by_category = {
        Network: items.filter((i) => i.category === 'Network'),
        Development: items.filter((i) => i.category === 'Development'),
        Graphics: items.filter((i) => i.category === 'Graphics'),
    }
    return {
        apps_by_category,
        scripts: [],
        ai_tools: [],
        recent_files: [],
        shortcuts: {},
        category_counts: { Network: 2, Development: 1, Graphics: 1 },
        source_counts: {
            'app:flatpak': 1,
            'app:snap': 1,
            'app:appimage': 1,
            'app:desktop': 1,
        },
        kind_counts: { app: 4 },
    }
}

function mockBrowse(payload = makeSectionsPayload()) {
    invokeMock.mockImplementation((cmd) => {
        if (cmd === 'browse_discoverable') return Promise.resolve(payload)
        if (cmd === 'launch_app') return Promise.resolve()
        if (cmd === 'record_action') return Promise.resolve()
        return Promise.resolve([])
    })
}

async function mountBrowse() {
    const wrapper = mount(OmnibarBrowse, {
        global: { plugins: [vuetify] },
        attachTo: document.body,
    })
    await flushPromises()
    await nextTick()
    return wrapper
}

describe('OmnibarBrowse (browse_discoverable payload)', () => {
    beforeEach(() => {
        invokeMock.mockReset()
    })

    it('renders an empty state when browse_discoverable returns no apps', async () => {
        mockBrowse({
            apps_by_category: {},
            scripts: [],
            ai_tools: [],
            recent_files: [],
            shortcuts: {},
            category_counts: {},
            source_counts: {},
            kind_counts: {},
        })
        const wrapper = await mountBrowse()
        expect(wrapper.text()).toContain('No categories found')
    })

    it('invokes browse_discoverable on mount instead of reading apps from useOmnibar', async () => {
        mockBrowse()
        const wrapper = await mountBrowse()
        const browseCalls = invokeMock.mock.calls.filter((c) => c[0] === 'browse_discoverable')
        expect(browseCalls.length).toBe(1)
        expect(wrapper.find('[data-testid="browse-chip-strip"]').exists()).toBe(true)
    })

    it('groups apps by their freedesktop categories from the unified payload', async () => {
        mockBrowse()
        const wrapper = await mountBrowse()
        const html = wrapper.html()
        expect(html).toContain('Network')
        expect(html).toContain('Development')
        expect(html).toContain('Graphics')
        expect(html).toContain('Chrome')
        expect(html).toContain('Firefox')
        expect(html).toContain('Code')
        expect(html).toContain('Figma')
    })

    it('renders source badges for Flatpak, Snap, AppImage, Nix, and Desktop entries', async () => {
        const payload = makeSectionsPayload()
        payload.apps_by_category.Nix = [makeItem('nixapp', 'NixApp', 'nixapp', 'Tools', 'nix')]
        payload.apps_by_category.Tools = payload.apps_by_category.Nix
        payload.category_counts.Tools = 1
        payload.source_counts['app:nix'] = 1
        mockBrowse(payload)
        const wrapper = await mountBrowse()
        const html = wrapper.html()
        expect(html).toContain('source-flatpak')
        expect(html).toContain('source-snap')
        expect(html).toContain('source-appimage')
        expect(html).toContain('source-nix')
        expect(html).toContain('source-desktop')
    })

    it('falls back to Uncategorized for apps without a category (bucket "Other")', async () => {
        const payload = makeSectionsPayload()
        const misc = makeItem('mystery', 'Mystery App', 'mystery', null, 'desktop')
        payload.apps_by_category.Other = [misc]
        payload.category_counts.Other = 1
        payload.source_counts['app:desktop'] = 2
        mockBrowse(payload)
        const wrapper = await mountBrowse()
        expect(wrapper.text()).toContain('Uncategorized')
        expect(wrapper.text()).toContain('Mystery App')
    })

    it('emits close event when the back button is pressed', async () => {
        mockBrowse()
        const wrapper = await mountBrowse()
        const backBtn = wrapper.find('.back-btn')
        expect(backBtn.exists()).toBe(true)
        await backBtn.trigger('click')
        expect(wrapper.emitted('close')).toBeTruthy()
    })

    it('launches the app via launch_app when a row is clicked', async () => {
        mockBrowse()
        const wrapper = await mountBrowse()
        const items = wrapper.findAll('.browse-app-item')
        expect(items.length).toBeGreaterThan(0)
        await items[0].trigger('click')
        await flushPromises()
        const launchCalls = invokeMock.mock.calls.filter((c) => c[0] === 'launch_app')
        expect(launchCalls.length).toBe(1)
        expect(typeof launchCalls[0][1].execCmd).toBe('string')
        expect(launchCalls[0][1].execCmd.length).toBeGreaterThan(0)
        const recordCalls = invokeMock.mock.calls.filter((c) => c[0] === 'record_action')
        expect(recordCalls.length).toBeGreaterThan(0)
    })

    it('renders category counts in headers', async () => {
        mockBrowse()
        const wrapper = await mountBrowse()
        const html = wrapper.html()
        expect(html).toContain('class="category-count"')
        const counts = wrapper.findAll('.category-count').map((c) => Number(c.text()))
        expect(counts).toContain(2)
        expect(counts).toContain(1)
    })

    it('renders category chips with counts sourced from category_counts', async () => {
        mockBrowse()
        const wrapper = await mountBrowse()
        const networkChip = wrapper.find('[data-testid="chip-category-Network"]')
        expect(networkChip.exists()).toBe(true)
        expect(networkChip.text()).toContain('Network')
        expect(networkChip.text()).toContain('2')
        const devChip = wrapper.find('[data-testid="chip-category-Development"]')
        expect(devChip.text()).toContain('1')
    })

    it('renders source chips derived from source_counts', async () => {
        mockBrowse()
        const wrapper = await mountBrowse()
        expect(wrapper.find('[data-testid="chip-source-flatpak"]').exists()).toBe(true)
        expect(wrapper.find('[data-testid="chip-source-snap"]').exists()).toBe(true)
        expect(wrapper.find('[data-testid="chip-source-appimage"]').exists()).toBe(true)
        expect(wrapper.find('[data-testid="chip-source-desktop"]').exists()).toBe(true)
    })

    it('filters visible sections to a single category when its chip is clicked', async () => {
        mockBrowse()
        const wrapper = await mountBrowse()
        const networkChip = wrapper.find('[data-testid="chip-category-Network"]')
        await networkChip.trigger('click')
        await nextTick()
        const sections = wrapper.findAll('[data-testid^="category-section-"]')
        const keys = sections.map((s) => s.attributes('data-testid'))
        expect(keys).toContain('category-section-Network')
        expect(keys).not.toContain('category-section-Development')
        expect(keys).not.toContain('category-section-Graphics')
    })

    it('clears the category filter when the active chip is clicked again', async () => {
        mockBrowse()
        const wrapper = await mountBrowse()
        const networkChip = wrapper.find('[data-testid="chip-category-Network"]')
        await networkChip.trigger('click')
        await nextTick()
        await networkChip.trigger('click')
        await nextTick()
        const sections = wrapper.findAll('[data-testid^="category-section-"]')
        const keys = sections.map((s) => s.attributes('data-testid'))
        expect(keys).toContain('category-section-Network')
        expect(keys).toContain('category-section-Development')
        expect(keys).toContain('category-section-Graphics')
    })

    it('filters visible apps to a single source when its chip is clicked', async () => {
        mockBrowse()
        const wrapper = await mountBrowse()
        const flatpakChip = wrapper.find('[data-testid="chip-source-flatpak"]')
        await flatpakChip.trigger('click')
        await nextTick()
        const items = wrapper.findAll('.browse-app-item')
        const itemIds = items.map((i) => i.attributes('data-testid'))
        expect(itemIds).toContain('browse-app-chrome')
        expect(itemIds).not.toContain('browse-app-firefox')
        expect(itemIds).not.toContain('browse-app-code')
        expect(itemIds).not.toContain('browse-app-figma')
    })

    it('filters by both category and source simultaneously', async () => {
        mockBrowse()
        const wrapper = await mountBrowse()
        const networkChip = wrapper.find('[data-testid="chip-category-Network"]')
        const snapChip = wrapper.find('[data-testid="chip-source-snap"]')
        await networkChip.trigger('click')
        await snapChip.trigger('click')
        await nextTick()
        const items = wrapper.findAll('.browse-app-item')
        const itemIds = items.map((i) => i.attributes('data-testid'))
        expect(itemIds).toContain('browse-app-firefox')
        expect(itemIds).not.toContain('browse-app-chrome')
    })

    it('hides the chip strip when no apps are returned', async () => {
        mockBrowse({
            apps_by_category: {},
            scripts: [],
            ai_tools: [],
            recent_files: [],
            shortcuts: {},
            category_counts: {},
            source_counts: {},
            kind_counts: {},
        })
        const wrapper = await mountBrowse()
        expect(wrapper.find('[data-testid="browse-chip-strip"]').exists()).toBe(false)
    })
})