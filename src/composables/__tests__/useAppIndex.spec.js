import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'

const invokeMock = vi.fn()
const listenMock = vi.fn(() => Promise.resolve(() => {}))

vi.mock('@tauri-apps/api/core', () => ({
    invoke: (...args) => invokeMock(...args),
}))

vi.mock('@tauri-apps/api/event', () => ({
    listen: (...args) => listenMock(...args),
}))

import {
    ALIAS_VALIDATION_REGEX,
    useAppIndex,
    _resetAppIndexForTests,
} from '../useAppIndex'

function makeStatus(overrides = {}) {
    return {
        total_apps: 3,
        indexed_at: 1_700_000_000,
        truncated: false,
        disabled_sources: ['flatpak'],
        sources_with_counts: [
            ['desktop', 2],
            ['flatpak', 1],
        ],
        ...overrides,
    }
}

describe('useAppIndex – alias validation regex', () => {
    it('exposes the documented ALIAS_VALIDATION_REGEX', () => {
        expect(ALIAS_VALIDATION_REGEX).toBeInstanceOf(RegExp)
        expect(ALIAS_VALIDATION_REGEX.source).toBe('^[A-Za-z0-9_. -]{1,32}$')
    })

    it('rejects empty strings', () => {
        const { isValidAlias } = useAppIndex()
        expect(isValidAlias('')).toBe(false)
    })

    it('rejects strings longer than 32 characters', () => {
        const { isValidAlias } = useAppIndex()
        expect(isValidAlias('a'.repeat(33))).toBe(false)
        expect(isValidAlias('a'.repeat(32))).toBe(true)
    })

    it('rejects shell meta characters', () => {
        const { isValidAlias } = useAppIndex()
        expect(isValidAlias('rm -rf /')).toBe(false)
        expect(isValidAlias('a;b')).toBe(false)
        expect(isValidAlias('a&b')).toBe(false)
        expect(isValidAlias('a|b')).toBe(false)
        expect(isValidAlias('a$b')).toBe(false)
        expect(isValidAlias('a`b`')).toBe(false)
        expect(isValidAlias('$(whoami)')).toBe(false)
        expect(isValidAlias('a>b')).toBe(false)
        expect(isValidAlias('a<b')).toBe(false)
    })

    it('rejects path separators', () => {
        const { isValidAlias } = useAppIndex()
        expect(isValidAlias('foo/bar')).toBe(false)
        expect(isValidAlias('foo\\bar')).toBe(false)
    })

    it('rejects non-ASCII characters', () => {
        const { isValidAlias } = useAppIndex()
        expect(isValidAlias('héllo')).toBe(false)
    })

    it('rejects non-string inputs', () => {
        const { isValidAlias } = useAppIndex()
        expect(isValidAlias(null)).toBe(false)
        expect(isValidAlias(undefined)).toBe(false)
        expect(isValidAlias(42)).toBe(false)
        expect(isValidAlias({})).toBe(false)
        expect(isValidAlias([])).toBe(false)
    })

    it('accepts typical alias shapes', () => {
        const { isValidAlias } = useAppIndex()
        expect(isValidAlias('browser')).toBe(true)
        expect(isValidAlias('g cal')).toBe(true)
        expect(isValidAlias('g_cal')).toBe(true)
        expect(isValidAlias('g-cal')).toBe(true)
        expect(isValidAlias('g.cal')).toBe(true)
        expect(isValidAlias('Slack123')).toBe(true)
    })
})

describe('useAppIndex – disabled sources normalization', () => {
    it('keeps only known source labels, lowercased and unique', () => {
        const { normalizeDisabledSources } = useAppIndex()
        const out = normalizeDisabledSources(['Flatpak', 'desktop', 'desktop', 'garbage', 'snap'])
        expect(out).toEqual(['flatpak', 'desktop', 'snap'])
    })

    it('returns an empty list when given garbage or non-array input', () => {
        const { normalizeDisabledSources } = useAppIndex()
        expect(normalizeDisabledSources(['unknown', 'nope'])).toEqual([])
        expect(normalizeDisabledSources(null)).toEqual([])
        expect(normalizeDisabledSources('flatpak')).toEqual([])
    })
})

describe('useAppIndex – command invocations', () => {
    beforeEach(() => {
        invokeMock.mockReset()
        listenMock.mockReset()
        listenMock.mockImplementation(() => Promise.resolve(() => {}))
        invokeMock.mockImplementation(() => Promise.resolve([]))
        _resetAppIndexForTests()
    })

    afterEach(() => {
        _resetAppIndexForTests()
    })

    it('listAppIndexStatus invokes get_app_index_status and stores the result', async () => {
        const status = makeStatus()
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'get_app_index_status') return Promise.resolve(status)
            return Promise.resolve([])
        })
        const ctx = useAppIndex()
        const result = await ctx.listAppIndexStatus()
        expect(invokeMock).toHaveBeenCalledWith('get_app_index_status')
        expect(result.total_apps).toBe(3)
        expect(ctx.appIndexStatus.value.total_apps).toBe(3)
        expect(ctx.appIndexStatus.value.disabled_sources).toEqual(['flatpak'])
        expect(ctx.disabledSources.value).toEqual(['flatpak'])
    })

    it('setDisabledSources sends a normalized list and stores the response', async () => {
        const status = makeStatus({ disabled_sources: ['flatpak', 'snap'] })
        invokeMock.mockImplementation((cmd, args) => {
            if (cmd === 'set_disabled_sources') {
                expect(args.sources).toEqual(['flatpak', 'snap'])
                return Promise.resolve(status)
            }
            return Promise.resolve([])
        })
        const ctx = useAppIndex()
        const result = await ctx.setDisabledSources(['Flatpak', 'snap', 'nope'])
        expect(result.disabled_sources).toEqual(['flatpak', 'snap'])
        expect(ctx.disabledSources.value).toEqual(['flatpak', 'snap'])
    })

    it('setAppAlias invokes set_app_aliases with camelCase appId', async () => {
        invokeMock.mockImplementation((cmd, args) => {
            if (cmd === 'set_app_aliases') {
                expect(args.appId).toBe('google-chrome')
                expect(args.aliases).toEqual(['browser', 'web'])
                return Promise.resolve(null)
            }
            return Promise.resolve([])
        })
        const ctx = useAppIndex()
        await ctx.setAppAlias('google-chrome', ['browser', 'web'])
        expect(invokeMock).toHaveBeenCalledWith(
            'set_app_aliases',
            expect.objectContaining({ appId: 'google-chrome', aliases: ['browser', 'web'] }),
        )
    })

    it('setAppAlias rejects invalid aliases without invoking the command', async () => {
        const ctx = useAppIndex()
        await expect(ctx.setAppAlias('chrome', ['rm -rf /'])).rejects.toThrow(/invalid alias/)
        expect(invokeMock).not.toHaveBeenCalled()
    })

    it('setAppAlias requires a non-empty appId', async () => {
        const ctx = useAppIndex()
        await expect(ctx.setAppAlias('', ['browser'])).rejects.toThrow()
        expect(invokeMock).not.toHaveBeenCalled()
    })

    it('bulkSetAliases sends snake_case entries for each appId', async () => {
        invokeMock.mockImplementation((cmd, args) => {
            if (cmd === 'bulk_set_aliases') {
                expect(args.entries).toEqual([
                    { app_id: 'google-chrome', aliases: ['browser'] },
                    { app_id: 'code', aliases: ['editor', 'ide'] },
                ])
                return Promise.resolve(null)
            }
            return Promise.resolve([])
        })
        const ctx = useAppIndex()
        await ctx.bulkSetAliases({
            'google-chrome': ['browser'],
            'code': ['editor', 'ide'],
        })
        expect(invokeMock).toHaveBeenCalledWith(
            'bulk_set_aliases',
            expect.objectContaining({
                entries: expect.arrayContaining([
                    expect.objectContaining({ app_id: 'google-chrome' }),
                ]),
            }),
        )
    })

    it('bulkSetAliases rejects when any alias is invalid', async () => {
        const ctx = useAppIndex()
        await expect(
            ctx.bulkSetAliases({ chrome: ['ok'], code: ['rm -rf /'] }),
        ).rejects.toThrow(/invalid alias/)
        expect(invokeMock).not.toHaveBeenCalled()
    })

    it('bulkSetAliases rejects non-object input', async () => {
        const ctx = useAppIndex()
        await expect(ctx.bulkSetAliases(null)).rejects.toThrow()
        expect(invokeMock).not.toHaveBeenCalled()
    })

    it('listAppIndexStatus propagates backend errors', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'get_app_index_status') return Promise.reject('boom')
            return Promise.resolve([])
        })
        const ctx = useAppIndex()
        await expect(ctx.listAppIndexStatus()).rejects.toBe('boom')
    })
})

describe('useAppIndex – subscribeIndexUpdates', () => {
    beforeEach(() => {
        invokeMock.mockReset()
        listenMock.mockReset()
        listenMock.mockImplementation(() => Promise.resolve(() => {}))
        invokeMock.mockImplementation(() => Promise.resolve([]))
        _resetAppIndexForTests()
    })

    afterEach(() => {
        _resetAppIndexForTests()
    })

    it('subscribes to both aliases-updated and app-index-status-updated', async () => {
        const listeners = {}
        listenMock.mockImplementation((event, handler) => {
            listeners[event] = handler
            return Promise.resolve(() => {})
        })
        const ctx = useAppIndex()
        const off = await ctx.subscribeIndexUpdates()
        expect(listeners['aliases-updated']).toBeDefined()
        expect(listeners['app-index-status-updated']).toBeDefined()
        expect(typeof off).toBe('function')
    })

    it('merges aliases-updated payload into aliasesByAppId', async () => {
        const listeners = {}
        listenMock.mockImplementation((event, handler) => {
            listeners[event] = handler
            return Promise.resolve(() => {})
        })
        const ctx = useAppIndex()
        await ctx.subscribeIndexUpdates()
        await listeners['aliases-updated']({
            payload: { app_id: 'google-chrome', aliases: ['browser', 'web'] },
        })
        expect(ctx.aliasesByAppId.value['google-chrome']).toEqual(['browser', 'web'])
    })

    it('app-index-status-updated payload updates the status ref and disabledSources', async () => {
        const listeners = {}
        listenMock.mockImplementation((event, handler) => {
            listeners[event] = handler
            return Promise.resolve(() => {})
        })
        const ctx = useAppIndex()
        await ctx.subscribeIndexUpdates()
        await listeners['app-index-status-updated']({
            payload: {
                status: makeStatus({
                    total_apps: 7,
                    disabled_sources: ['desktop', 'flatpak'],
                }),
            },
        })
        expect(ctx.appIndexStatus.value.total_apps).toBe(7)
        expect(ctx.disabledSources.value).toEqual(['desktop', 'flatpak'])
    })

    it('handles app-index-status-updated payload without a status wrapper', async () => {
        const listeners = {}
        listenMock.mockImplementation((event, handler) => {
            listeners[event] = handler
            return Promise.resolve(() => {})
        })
        const ctx = useAppIndex()
        await ctx.subscribeIndexUpdates()
        await listeners['app-index-status-updated']({
            payload: makeStatus({ total_apps: 9 }),
        })
        expect(ctx.appIndexStatus.value.total_apps).toBe(9)
    })

    it('aliases-updated handles payload with multiple entries', async () => {
        const listeners = {}
        listenMock.mockImplementation((event, handler) => {
            listeners[event] = handler
            return Promise.resolve(() => {})
        })
        const ctx = useAppIndex()
        await ctx.subscribeIndexUpdates()
        await listeners['aliases-updated']({
            payload: {
                entries: [
                    { app_id: 'google-chrome', aliases: ['browser'] },
                    { app_id: 'code', aliases: ['editor', 'ide'] },
                ],
            },
        })
        expect(ctx.aliasesByAppId.value['google-chrome']).toEqual(['browser'])
        expect(ctx.aliasesByAppId.value['code']).toEqual(['editor', 'ide'])
    })

    it('ignores malformed payload entries', async () => {
        const listeners = {}
        listenMock.mockImplementation((event, handler) => {
            listeners[event] = handler
            return Promise.resolve(() => {})
        })
        const ctx = useAppIndex()
        await ctx.subscribeIndexUpdates()
        await listeners['aliases-updated']({ payload: { app_id: 42, aliases: [] } })
        await listeners['aliases-updated']({ payload: null })
        expect(ctx.aliasesByAppId.value).toEqual({})
    })

    it('unlisten detaches both event listeners', async () => {
        const aliasUnlisten = vi.fn()
        const statusUnlisten = vi.fn()
        let call = 0
        listenMock.mockImplementation(() => {
            call += 1
            return Promise.resolve(call === 1 ? aliasUnlisten : statusUnlisten)
        })
        const ctx = useAppIndex()
        const off = await ctx.subscribeIndexUpdates()
        off()
        expect(aliasUnlisten).toHaveBeenCalled()
        expect(statusUnlisten).toHaveBeenCalled()
    })
})
