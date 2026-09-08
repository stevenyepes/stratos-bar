import { mount, flushPromises } from '@vue/test-utils'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { createVuetify } from 'vuetify'
import * as components from 'vuetify/components'
import * as directives from 'vuetify/directives'

vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn((cmd) => Promise.resolve(cmd === 'get_recent_actions' ? [] : undefined)),
    convertFileSrc: vi.fn((path) => path),
}))

vi.mock('@tauri-apps/api/window', () => ({
    getCurrentWindow: () => ({
        hide: vi.fn(),
        setSize: vi.fn(),
        setFocus: vi.fn(),
    }),
    currentMonitor: vi.fn().mockResolvedValue(null),
}))

vi.mock('@tauri-apps/api/dpi', () => ({
    LogicalSize: class LogicalSize {
        constructor(width, height) {
            this.width = width
            this.height = height
        }
    },
}))

vi.mock('@tauri-apps/api/path', () => ({
    homeDir: vi.fn().mockResolvedValue('/home/test'),
}))

vi.mock('../../../theme', () => ({
    applyTheme: vi.fn(),
}))

vi.mock('../../../skills', () => ({
    default: {
        match: vi.fn().mockReturnValue(null),
    },
}))

vi.mock('vuetify', async (importOriginal) => {
    const actual = await importOriginal()
    return {
        ...actual,
        useTheme: () => ({ themes: { value: { dark: { colors: {} } } } }),
    }
})

import { invoke } from '@tauri-apps/api/core'
import { useOmnibar } from '../../../composables/useOmnibar'
import OmnibarSearch from '../OmnibarSearch.vue'

const vuetify = createVuetify({
    components,
    directives,
})

describe('OmnibarSearch', () => {
    beforeEach(() => {
        vi.clearAllMocks()
        // Reset the singleton omnibar state shared across tests
        const { query, selectedIndex, showSettings, recentActions, uiState } = useOmnibar()
        query.value = ''
        selectedIndex.value = 0
        showSettings.value = false
        recentActions.value = []
        uiState.value = 'idle'
    })

    it('replays a recent app action with the original app id, not undefined', async () => {
        const { recentActions } = useOmnibar()
        recentActions.value = [
            { id: 'app:firefox.desktop', kind: 'app', content: '/usr/bin/firefox', name: 'Firefox' },
        ]

        const wrapper = mount(OmnibarSearch, {
            global: {
                plugins: [vuetify],
            },
        })

        // Settings item is index 0, the recent action is index 1
        const recentActionItem = wrapper.findAll('.result-item')[1]
        expect(recentActionItem.text()).toContain('Firefox')

        await recentActionItem.trigger('click')
        await flushPromises()

        expect(invoke).toHaveBeenCalledWith('launch_app', { execCmd: '/usr/bin/firefox' })
        expect(invoke).toHaveBeenCalledWith('record_action', {
            action: expect.objectContaining({
                id: 'app:firefox.desktop',
                content: '/usr/bin/firefox',
            }),
        })
    })

    it('does not regress script replay: reconstructs from content/name and records it', async () => {
        const { recentActions } = useOmnibar()
        recentActions.value = [
            { id: 'script:build', kind: 'script', content: '/home/user/build.sh', name: 'build' },
        ]

        const wrapper = mount(OmnibarSearch, {
            global: {
                plugins: [vuetify],
            },
        })

        const recentActionItem = wrapper.findAll('.result-item')[1]
        await recentActionItem.trigger('click')
        await flushPromises()

        expect(invoke).toHaveBeenCalledWith('execute_script', expect.objectContaining({ path: '/home/user/build.sh' }))
        expect(invoke).toHaveBeenCalledWith('record_action', {
            action: expect.objectContaining({
                id: 'script:build',
                content: '/home/user/build.sh',
            }),
        })
    })
})
