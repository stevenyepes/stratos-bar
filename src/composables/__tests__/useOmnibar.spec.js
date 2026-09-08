import { describe, it, expect, vi, beforeEach } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn().mockResolvedValue(undefined)
}))

vi.mock('@tauri-apps/api/window', () => ({
    getCurrentWindow: () => ({
        hide: vi.fn(),
        setSize: vi.fn(),
        setFocus: vi.fn()
    }),
    currentMonitor: vi.fn().mockResolvedValue(null)
}))

vi.mock('@tauri-apps/api/dpi', () => ({
    LogicalSize: class LogicalSize {
        constructor(width, height) {
            this.width = width
            this.height = height
        }
    }
}))

vi.mock('@tauri-apps/api/path', () => ({
    homeDir: vi.fn().mockResolvedValue('/home/test')
}))

vi.mock('../../theme', () => ({
    applyTheme: vi.fn()
}))

vi.mock('../../skills', () => ({
    default: {
        match: vi.fn().mockReturnValue(null)
    }
}))

vi.mock('vuetify', () => ({
    useTheme: () => ({
        themes: { value: { dark: { colors: {} } } }
    })
}))

import { invoke } from '@tauri-apps/api/core'
import { useOmnibar } from '../useOmnibar'

describe('useOmnibar', () => {
    beforeEach(() => {
        vi.clearAllMocks()
    })

    describe('recordAction', () => {
        it('keys app actions by app id, not exec', async () => {
            const { recordAction } = useOmnibar()

            await recordAction({ id: 'firefox.desktop', exec: '/usr/bin/firefox', name: 'Firefox' })

            expect(invoke).toHaveBeenCalledWith('record_action', {
                action: expect.objectContaining({
                    id: 'app:firefox.desktop',
                    content: '/usr/bin/firefox'
                })
            })
        })
    })
})
