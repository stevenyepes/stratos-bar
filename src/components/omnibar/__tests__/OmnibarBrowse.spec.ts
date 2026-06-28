import { mount } from '@vue/test-utils'
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

function makeApp(id, name, exec, categories = [], source = 'desktop') {
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
        aliases: [],
    }
}

async function getOmnibar() {
    const mod = await import('../../../composables/useOmnibar')
    return mod
}

describe('OmnibarBrowse', () => {
    let omnibar

    beforeEach(async () => {
        invokeMock.mockReset()
        invokeMock.mockImplementation(() => Promise.resolve([]))
        const mod = await getOmnibar()
        mod._resetOmnibarForTests()
        omnibar = mod.useOmnibar()
        omnibar.apps.value = [
            makeApp('chrome', 'Chrome', 'chrome', ['Network'], 'flatpak'),
            makeApp('firefox', 'Firefox', 'firefox', ['Network'], 'snap'),
            makeApp('code', 'Code', 'code', ['Development'], 'appimage'),
            makeApp('figma', 'Figma', 'figma', ['Graphics'], 'desktop'),
        ]
    })

    it('renders an empty state when no apps are present', async () => {
        omnibar.apps.value = []
        const wrapper = mount(OmnibarBrowse, {
            global: { plugins: [vuetify] },
            attachTo: document.body,
        })
        await nextTick()
        expect(wrapper.text()).toContain('No categories found')
    })

    it('groups apps by their freedesktop categories', async () => {
        const wrapper = mount(OmnibarBrowse, {
            global: { plugins: [vuetify] },
            attachTo: document.body,
        })
        await nextTick()
        const html = wrapper.html()
        expect(html).toContain('Network')
        expect(html).toContain('Development')
        expect(html).toContain('Graphics')
        expect(html).toContain('Chrome')
        expect(html).toContain('Firefox')
        expect(html).toContain('Code')
        expect(html).toContain('Figma')
    })

    it('renders source badges for Flatpak, Snap, and AppImage entries', async () => {
        const wrapper = mount(OmnibarBrowse, {
            global: { plugins: [vuetify] },
            attachTo: document.body,
        })
        await nextTick()
        const html = wrapper.html()
        expect(html).toContain('source-flatpak')
        expect(html).toContain('source-snap')
        expect(html).toContain('source-appimage')
    })

    it('falls back to Uncategorized for apps without categories', async () => {
        omnibar.apps.value = [makeApp('mystery', 'Mystery App', 'mystery', [], 'desktop')]
        const wrapper = mount(OmnibarBrowse, {
            global: { plugins: [vuetify] },
            attachTo: document.body,
        })
        await nextTick()
        expect(wrapper.text()).toContain('Uncategorized')
        expect(wrapper.text()).toContain('Mystery App')
    })

    it('emits close event when the back button is pressed', async () => {
        const wrapper = mount(OmnibarBrowse, {
            global: { plugins: [vuetify] },
            attachTo: document.body,
        })
        await nextTick()
        const backBtn = wrapper.find('.back-btn')
        expect(backBtn.exists()).toBe(true)
        await backBtn.trigger('click')
        expect(wrapper.emitted('close')).toBeTruthy()
    })

    it('launches the app via launch_app when a row is clicked', async () => {
        invokeMock.mockImplementation((cmd, args) => {
            if (cmd === 'launch_app') return Promise.resolve()
            return Promise.resolve([])
        })
        const wrapper = mount(OmnibarBrowse, {
            global: { plugins: [vuetify] },
            attachTo: document.body,
        })
        await nextTick()
        const items = wrapper.findAll('.browse-app-item')
        expect(items.length).toBeGreaterThan(0)
        await items[0].trigger('click')
        await nextTick()
        const launchCalls = invokeMock.mock.calls.filter((c) => c[0] === 'launch_app')
        expect(launchCalls.length).toBe(1)
        expect(typeof launchCalls[0][1].execCmd).toBe('string')
        expect(launchCalls[0][1].execCmd.length).toBeGreaterThan(0)
        const recordCalls = invokeMock.mock.calls.filter((c) => c[0] === 'record_action')
        expect(recordCalls.length).toBeGreaterThan(0)
    })

    it('renders category counts in headers', async () => {
        const wrapper = mount(OmnibarBrowse, {
            global: { plugins: [vuetify] },
            attachTo: document.body,
        })
        await nextTick()
        const html = wrapper.html()
        expect(html).toContain('class="category-count"')
        const counts = wrapper.findAll('.category-count').map((c) => Number(c.text()))
        expect(counts).toContain(2)
        expect(counts).toContain(1)
    })
})