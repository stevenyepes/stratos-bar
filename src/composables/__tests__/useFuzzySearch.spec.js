import { describe, it, expect } from 'vitest'

import { scoreEntry, rankEntries, filterByCategory } from '../useFuzzySearch'

function makeEntry(overrides = {}) {
    return {
        id: 'chrome',
        name: 'Chrome',
        exec: 'chrome',
        source: 'desktop',
        categories: [],
        aliases: [],
        ...overrides,
    }
}

function makeDiscoverableEntry(overrides = {}) {
    return {
        id: 'chrome',
        name: 'Chrome',
        exec: 'chrome',
        source: 'desktop',
        category: null,
        categories: [],
        ...overrides,
    }
}

describe('useFuzzySearch – scoreEntry precedence', () => {
    it('ranks prefix match above contains match', () => {
        const prefix = scoreEntry(makeEntry({ id: 'figma', name: 'Figma' }), 'fig')
        const contains = scoreEntry(
            makeEntry({ id: 'config', name: 'AppConfig' }),
            'fig',
        )
        expect(prefix).toBeGreaterThan(contains)
        expect(contains).toBeGreaterThan(0)
    })

    it('ranks contains match above subsequence match', () => {
        const contains = scoreEntry(
            makeEntry({ id: 'custom', name: 'Customize' }),
            'tom',
        )
        const subseq = scoreEntry(
            makeEntry({ id: 'tabs', name: 'Tabs' }),
            'tbs',
        )
        expect(contains).toBeGreaterThan(subseq)
        expect(subseq).toBeGreaterThan(0)
    })

    it('returns zero for non-matching entry', () => {
        const score = scoreEntry(
            makeEntry({ id: 'chrome', name: 'Chrome' }),
            'zzzzz',
        )
        expect(score).toBe(0)
    })

    it('treats empty query as zero except for frequency decay', () => {
        const entry = makeEntry({ id: 'chrome', name: 'Chrome' })
        const withoutHistory = scoreEntry(entry, '')
        expect(withoutHistory).toBe(0)

        const withHistory = scoreEntry(
            makeEntry({
                id: 'chrome',
                name: 'Chrome',
                launch_count: 5,
                last_launched_at: Date.now(),
            }),
            '',
        )
        expect(withHistory).toBeGreaterThan(0)
    })

    it('case insensitive', () => {
        const upper = scoreEntry(makeEntry({ name: 'Figma' }), 'FIG')
        const lower = scoreEntry(makeEntry({ name: 'Figma' }), 'fig')
        expect(upper).toBe(lower)
    })
})

describe('useFuzzySearch – ranks_chr_subsequence (AC-1)', () => {
    const entries = [
        makeEntry({ id: 'chrome', name: 'Chrome' }),
        makeEntry({ id: 'chromium', name: 'Chromium' }),
        makeEntry({ id: 'code', name: 'Code' }),
        makeEntry({ id: 'cursor', name: 'Cursor' }),
        makeEntry({ id: 'figma', name: 'Figma' }),
    ]

    it('returns Chrome before Chromium for query "chr"', () => {
        const ranked = rankEntries(entries, 'chr', 10)
        expect(ranked.length).toBe(2)
        expect(ranked[0].entry.id).toBe('chrome')
        expect(ranked[1].entry.id).toBe('chromium')
    })

    it('excludes non-matching entries', () => {
        const ranked = rankEntries(entries, 'chr', 10)
        const ids = ranked.map((r) => r.entry.id)
        expect(ids).not.toContain('code')
        expect(ids).not.toContain('cursor')
        expect(ids).not.toContain('figma')
    })

    it('returns empty list when nothing matches', () => {
        const ranked = rankEntries(entries, 'zzzz', 10)
        expect(ranked).toEqual([])
    })
})

describe('useFuzzySearch – ranks_alias_above_name (AC-2)', () => {
    it('prefers an aliased match over a higher-fuzzy name match', () => {
        const aliased = makeEntry({
            id: 'google-chrome',
            name: 'Browser',
            aliases: ['browser'],
        })
        const nameMatch = makeEntry({
            id: 'browser-app',
            name: 'Browser Helper',
            aliases: [],
        })

        const ranked = rankEntries([aliased, nameMatch], 'browser', 10)
        expect(ranked[0].entry.id).toBe('google-chrome')
        expect(ranked[0].score).toBeGreaterThan(ranked[1].score)
    })

    it('injects aliases from ctx when entry has none', () => {
        const entry = makeEntry({ id: 'google-chrome', name: 'Google Chrome' })
        const ranked = rankEntries(
            [entry],
            'browser',
            10,
            { aliases: { 'google-chrome': ['browser'] } },
        )
        expect(ranked.length).toBe(1)
        expect(ranked[0].score).toBeGreaterThan(0)
    })

    it('exact alias beats a name-prefix match', () => {
        const aliased = makeEntry({
            id: 'editor',
            name: 'X',
            aliases: ['editor'],
        })
        const namePrefix = makeEntry({
            id: 'editor-online',
            name: 'Editor Online',
            aliases: [],
        })
        const ranked = rankEntries([aliased, namePrefix], 'editor', 10)
        expect(ranked[0].entry.id).toBe('editor')
    })
})

describe('useFuzzySearch – ranks_top_apps_by_decay (AC-5)', () => {
    it('orders results by frequency × exp(-age_days/14)', () => {
        const now = 1_700_000_000_000
        const day = 86_400_000
        const entries = [
            makeEntry({
                id: 'old-but-popular',
                name: 'Old',
                launch_count: 100,
                last_launched_at: now - 60 * day,
            }),
            makeEntry({
                id: 'fresh-popular',
                name: 'Fresh',
                launch_count: 10,
                last_launched_at: now - 1 * day,
            }),
            makeEntry({
                id: 'stale',
                name: 'Stale',
                launch_count: 5,
                last_launched_at: now - 30 * day,
            }),
        ]
        const ranked = rankEntries(entries, '', 10, { now })
        expect(ranked.length).toBe(3)
        expect(ranked[0].entry.id).toBe('fresh-popular')
        expect(ranked[0].score).toBeGreaterThan(ranked[1].score)
        expect(ranked[1].score).toBeGreaterThan(ranked[2].score)
    })
})

describe('useFuzzySearch – limit behavior', () => {
    it('caps the result list at the provided limit', () => {
        const entries = []
        for (let i = 0; i < 20; i += 1) {
            entries.push(makeEntry({ id: `chrome-${i}`, name: `Chrome ${i}` }))
        }
        const ranked = rankEntries(entries, 'chrome', 5)
        expect(ranked.length).toBe(5)
    })

    it('returns all matches when limit is omitted or invalid', () => {
        const entries = [
            makeEntry({ id: 'a', name: 'Alpha' }),
            makeEntry({ id: 'b', name: 'Beta' }),
        ]
        const ranked = rankEntries(entries, 'a')
        expect(ranked.length).toBe(2)
    })

    it('handles 5000-entry list well within 50ms', () => {
        const entries = []
        for (let i = 0; i < 5000; i += 1) {
            entries.push(makeEntry({ id: `app-${i}`, name: `App ${i}` }))
        }
        entries.push(makeEntry({ id: 'special', name: 'SpecialChrome' }))
        const start = performance.now()
        const ranked = rankEntries(entries, 'chrome', 100)
        const elapsed = performance.now() - start
        expect(ranked.length).toBeGreaterThan(0)
        expect(ranked[0].entry.id).toBe('special')
        expect(elapsed).toBeLessThan(50)
    })
})

describe('useFuzzySearch – filterByCategory', () => {
    const entries = [
        makeEntry({ id: 'chrome', name: 'Chrome', categories: ['Network', 'WebBrowser'] }),
        makeEntry({ id: 'code', name: 'Code', categories: ['Development'] }),
        makeEntry({ id: 'figma', name: 'Figma', categories: ['Graphics', 'Design'] }),
    ]

    it('keeps only entries whose categories contain the target', () => {
        const filtered = filterByCategory(entries, 'Development')
        expect(filtered.length).toBe(1)
        expect(filtered[0].id).toBe('code')
    })

    it('matches case-insensitively', () => {
        const filtered = filterByCategory(entries, 'network')
        expect(filtered.length).toBe(1)
        expect(filtered[0].id).toBe('chrome')
    })

    it('returns an empty array when nothing matches', () => {
        const filtered = filterByCategory(entries, 'Audio')
        expect(filtered).toEqual([])
    })

    it('returns all entries when category is empty/falsy', () => {
        expect(filterByCategory(entries, '').length).toBe(3)
        expect(filterByCategory(entries, null).length).toBe(3)
        expect(filterByCategory(entries, undefined).length).toBe(3)
    })

    it('falls back to singular category field if categories array missing', () => {
        const discoverable = [
            makeDiscoverableEntry({ id: 'chrome', name: 'Chrome', category: 'Network' }),
            makeDiscoverableEntry({ id: 'code', name: 'Code', category: 'Development' }),
        ]
        const filtered = filterByCategory(discoverable, 'Network')
        expect(filtered.length).toBe(1)
        expect(filtered[0].id).toBe('chrome')
    })

    it('returns an empty array for non-array input', () => {
        expect(filterByCategory(null, 'Network')).toEqual([])
        expect(filterByCategory(undefined, 'Network')).toEqual([])
        expect(filterByCategory('not-an-array', 'Network')).toEqual([])
    })

    it('does not mutate the input array', () => {
        const original = [...entries]
        filterByCategory(entries, 'Development')
        expect(entries).toEqual(original)
    })
})

describe('useFuzzySearch – rankEntries robustness', () => {
    it('handles empty entry list', () => {
        expect(rankEntries([], 'chrome')).toEqual([])
    })

    it('handles non-array entry list', () => {
        expect(rankEntries(null, 'chrome')).toEqual([])
        expect(rankEntries(undefined, 'chrome')).toEqual([])
    })

    it('treats entries without a name as zero score', () => {
        const noName = { id: 'x' }
        expect(scoreEntry(noName, 'xyz')).toBe(0)
    })

    it('ties broken deterministically by name', () => {
        const a = makeEntry({ id: 'a', name: 'Alpha' })
        const b = makeEntry({ id: 'b', name: 'Alpaca' })
        const ranked = rankEntries([b, a], 'al', 10)
        expect(ranked.map((r) => r.entry.id)).toEqual(['a', 'b'])
    })
})
