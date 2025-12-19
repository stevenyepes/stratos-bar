import { describe, it, expect, vi, beforeEach } from 'vitest'
import { MatugenSkill } from '../builtin/MatugenSkill'

// Mock dependencies
vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn().mockImplementation((cmd) => {
        if (cmd === 'get_config') return Promise.resolve({ theme: {} })
        return Promise.resolve()
    })
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({
    open: vi.fn()
}))

vi.mock('@tauri-apps/plugin-fs', () => ({
    readFile: vi.fn()
}))

vi.mock('../../theme', () => ({
    applyTheme: vi.fn()
}))

// Mock material-color-utilities
vi.mock('@material/material-color-utilities', () => ({
    themeFromSourceColor: vi.fn(() => ({
        schemes: {
            dark: {
                primary: 0xFF0000,
                secondary: 0x00FF00,
                surface: 0x0000FF,
                surfaceContainer: 0x0000AA,
                onSurface: 0xFFFFFF
            }
        }
    })),
    sourceColorFromImage: vi.fn(() => Promise.resolve(0xFF0000)),
    hexFromArgb: vi.fn((arg) => '#' + arg.toString(16))
}))

// Mock DOM/Browser APIs not present in JSDOM (or incomplete)
global.URL.createObjectURL = vi.fn(() => 'blob:test')
global.URL.revokeObjectURL = vi.fn()
global.Image = class {
    constructor() {
        setTimeout(() => {
            this.onload && this.onload()
        }, 10)
    }
}
global.window.dispatchEvent = vi.fn()

// Import mocked modules for asserting
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { readFile } from '@tauri-apps/plugin-fs'

describe('MatugenSkill', () => {

    beforeEach(() => {
        vi.clearAllMocks()
    })

    it('identifies relevant queries', () => {
        expect(MatugenSkill.match('change theme')).not.toBeNull()
        expect(MatugenSkill.match('generate colors')).not.toBeNull()
        expect(MatugenSkill.match('set wallpaper theme')).not.toBeNull()
        expect(MatugenSkill.match('matugen')).not.toBeNull()

        // Check score and structure
        const match = MatugenSkill.match('theme')
        expect(match.score).toBe(0.9)
        expect(match.preview).toContain('Pick an image')
    })

    it('ignores irrelevant queries', () => {
        expect(MatugenSkill.match('hello world')).toBeNull()
        expect(MatugenSkill.match('calculate 2+2')).toBeNull()
    })

    it('handles user cancellation', async () => {
        open.mockResolvedValue(null)

        const result = await MatugenSkill.execute({})
        expect(result).toBeNull()
        expect(open).toHaveBeenCalled()
        // Should NOT assume readFile is called if canceled
        expect(readFile).not.toHaveBeenCalled()
    })

    it('processing flow: generates theme and saves config', async () => {
        open.mockResolvedValue('/path/to/image.png')
        readFile.mockResolvedValue(new Uint8Array([1, 2, 3]))

        const result = await MatugenSkill.execute({})

        // 1. Check if file was read
        expect(readFile).toHaveBeenCalledWith('/path/to/image.png')

        // 2. Check if theme was saved
        expect(invoke).toHaveBeenCalledWith('get_config')
        expect(invoke).toHaveBeenCalledWith('save_config', expect.any(Object))

        // 3. Check if window event was dispatched
        expect(window.dispatchEvent).toHaveBeenCalledWith(expect.any(CustomEvent))

        // 4. Check if matugen command was launched
        expect(invoke).toHaveBeenCalledWith('launch_app', {
            execCmd: 'matugen image "/path/to/image.png"'
        })

        // 5. Verify return message
        expect(result).toBe('Theme updated from image.png')
    })
})
