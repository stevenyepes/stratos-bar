import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { CurrencySkill } from '../builtin/CurrencySkill'

describe('CurrencySkill', () => {
    beforeEach(() => {
        // Mock localStorage
        global.localStorage = {
            getItem: vi.fn(),
            setItem: vi.fn(),
            clear: vi.fn()
        }

        // Reset skill state
        CurrencySkill._rates = null
        CurrencySkill._lastFetch = 0
    })

    afterEach(() => {
        vi.clearAllMocks()
    })

    it('matches valid currency conversion queries', () => {
        const queries = [
            '100 usd to eur',
            '10 eur in gbp',
            'convert 50 jpy to usd',
            '100 $ to €'
        ]

        for (const q of queries) {
            const match = CurrencySkill.match(q)
            expect(match).not.toBeNull()
            expect(match.score).toBe(1.0)
            expect(match.data.amount).toBeGreaterThan(0)
            expect(match.data.from).toBeTruthy()
            expect(match.data.to).toBeTruthy()
        }
    })

    it('matches math expressions in currency queries', () => {
        const match = CurrencySkill.match('(10 + 20) usd to eur')
        expect(match).not.toBeNull()
        expect(match.data.amount).toBe(30)
    })

    it('defaults to local currency when destination is missing (Colombia locale)', () => {
        // We can't easily mock Intl completely, but we can check if our logic handles manual inputs if we exposed it,
        // or just trust the previous unit tests.
        // However, we want to verify the new mapping logic.
        // We can mock Intl.NumberFormat

        const originalIntl = global.Intl

        // Mock for Colombia
        global.Intl = {
            NumberFormat: class {
                resolvedOptions() {
                    return { locale: 'es-CO' }
                }
            }
        }

        const match = CurrencySkill.match('100 usd')
        expect(match.data.to).toBe('COP')

        // Restore
        global.Intl = originalIntl
    })

    it('defaults to local currency based on TimeZone when locale is generic (e.g. en-US in Colombia)', () => {
        const originalIntl = global.Intl

        // Mock: Locale is en-US (default), but TimeZone is America/Bogota
        global.Intl = {
            NumberFormat: class {
                resolvedOptions() { return { locale: 'en-US' } }
            },
            DateTimeFormat: class {
                resolvedOptions() { return { timeZone: 'America/Bogota' } }
            }
        }

        const match = CurrencySkill.match('100 usd')
        expect(match.data.to).toBe('COP')

        global.Intl = originalIntl
    })

    it('defaults to local currency when destination is missing (Default/Fallback)', () => {
        const originalIntl = global.Intl

        // Mock neutral environment (en-US, UTC)
        global.Intl = {
            NumberFormat: class { resolvedOptions() { return { locale: 'en-US' } } },
            DateTimeFormat: class { resolvedOptions() { return { timeZone: 'UTC' } } }
        }

        const match = CurrencySkill.match('100 eur')
        expect(match).not.toBeNull()
        expect(match.data.from).toBe('EUR')
        // fallback to USD because UTC isn't in our map
        expect(match.data.to).toBe('USD')

        const match2 = CurrencySkill.match('50 gbp')
        expect(match2).not.toBeNull()
        expect(match2.data.from).toBe('GBP')
        expect(match2.data.to).toBe('USD')

        global.Intl = originalIntl
    })

    it('returns null for truly invalid queries', () => {
        expect(CurrencySkill.match('hello world')).toBeNull()
        // '100 usd' is now valid, so we removed it from this test
        expect(CurrencySkill.match('open firefox')).toBeNull()
    })

    it('resolves common symbols', () => {
        expect(CurrencySkill._resolveCurrency('$')).toBe('USD')
        expect(CurrencySkill._resolveCurrency('€')).toBe('EUR')
        expect(CurrencySkill._resolveCurrency('£')).toBe('GBP')
        expect(CurrencySkill._resolveCurrency('KZT')).toBe('KZT') // Unknown but 3 chars
    })
})
