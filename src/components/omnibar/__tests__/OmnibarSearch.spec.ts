import { mount } from '@vue/test-utils'
import { describe, it, expect, beforeEach, vi } from 'vitest'
import OmnibarSearch from '../OmnibarSearch.vue'
import { createVuetify } from 'vuetify'
import * as components from 'vuetify/components'
import * as directives from 'vuetify/directives'

vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(),
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

const vuetify = createVuetify({ components, directives })

describe('OmnibarSearch', () => {
    let wrapper

    beforeEach(() => {
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
})