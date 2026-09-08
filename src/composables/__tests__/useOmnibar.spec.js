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

    describe('updateWindowSize - set_icon_scale', () => {
        // Module-level scale tracking is a singleton, so each test gets a
        // fresh module instance to avoid order-dependence between tests.
        beforeEach(() => {
            vi.resetModules()
        })

        const monitorWithScale = (scaleFactor) => ({
            scaleFactor,
            size: { width: 1920, height: 1080 }
        })

        it('calls set_icon_scale on the first call and updates apps', async () => {
            const { currentMonitor } = await import('@tauri-apps/api/window')
            currentMonitor.mockResolvedValue(monitorWithScale(1.5))
            const { invoke: freshInvoke } = await import('@tauri-apps/api/core')
            freshInvoke.mockResolvedValue(['app-a'])
            const { useOmnibar: freshUseOmnibar } = await import('../useOmnibar')

            const { updateWindowSize, apps } = freshUseOmnibar()
            await updateWindowSize()

            expect(freshInvoke).toHaveBeenCalledWith('set_icon_scale', { scale: 2 })
            expect(apps.value).toEqual(['app-a'])
        })

        it('does not call set_icon_scale again when the rounded scale is unchanged', async () => {
            const { currentMonitor } = await import('@tauri-apps/api/window')
            currentMonitor.mockResolvedValue(monitorWithScale(1.5))
            const { invoke: freshInvoke } = await import('@tauri-apps/api/core')
            freshInvoke.mockResolvedValue(['app-a'])
            const { useOmnibar: freshUseOmnibar } = await import('../useOmnibar')

            const { updateWindowSize, apps } = freshUseOmnibar()
            await updateWindowSize()

            freshInvoke.mockClear()
            freshInvoke.mockResolvedValue(['app-b'])

            await updateWindowSize()

            expect(freshInvoke).not.toHaveBeenCalledWith('set_icon_scale', expect.anything())
            // apps.value must be left untouched when the call is skipped
            expect(apps.value).toEqual(['app-a'])
        })

        it('calls set_icon_scale again when the rounded scale changes', async () => {
            const { currentMonitor } = await import('@tauri-apps/api/window')
            currentMonitor.mockResolvedValue(monitorWithScale(1.0))
            const { invoke: freshInvoke } = await import('@tauri-apps/api/core')
            freshInvoke.mockResolvedValue(['app-a'])
            const { useOmnibar: freshUseOmnibar } = await import('../useOmnibar')

            const { updateWindowSize, apps } = freshUseOmnibar()
            await updateWindowSize()

            currentMonitor.mockResolvedValue(monitorWithScale(2.0))
            freshInvoke.mockClear()
            freshInvoke.mockResolvedValue(['app-b'])

            await updateWindowSize()

            expect(freshInvoke).toHaveBeenCalledWith('set_icon_scale', { scale: 2 })
            expect(apps.value).toEqual(['app-b'])
        })
    })
})
