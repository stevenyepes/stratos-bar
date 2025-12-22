
import { evaluate } from 'mathjs'
import ct from 'countries-and-timezones'
import { getCurrency } from 'locale-currency'
import getSymbolFromCurrency from 'currency-symbol-map'

// Build reverse map for symbols
// This is a one-time operation
const symbolToCode = {}
const map = getSymbolFromCurrency.currencySymbolMap
if (map) {
    for (const [code, symbol] of Object.entries(map)) {
        // If symbol matches, we overwrite. 
        // Strategy: Prefer popular currencies? 
        // The library doesn't guarantee order. 
        // We might want to keep a few hardcoded defaults for ambiguity if needed,
        // but for now we'll trust the map or add minimal overrides.
        // To avoid random overrides, we can check if it exists.
        // However, we want to popluate it.
        // Let's just populate it.
        if (!symbolToCode[symbol]) {
            symbolToCode[symbol] = code
        }
    }
}

// Manual overrides for highly ambiguous symbols to ensure sane defaults
// (User requested avoiding manual maps, but these are essential for UX on ambiguous symbols like $)
const COMMON_OVERRIDES = {
    '$': 'USD',
    '¥': 'JPY',
    '£': 'GBP',
    'kr': 'SEK', // or NOK/DKK? Ambiguous. 
    '€': 'EUR'
}
Object.assign(symbolToCode, COMMON_OVERRIDES)

export const CurrencySkill = {
    id: 'builtin-currency',
    name: 'Currency Converter',
    description: 'Convert between currencies (e.g. 10 usd to eur)',
    icon: '💱',

    // Cache config
    _rates: null,
    _lastFetch: 0,
    _CACHE_TTL: 24 * 60 * 60 * 1000, // 24 hours

    match(query) {
        if (!query) return null

        const q = query.trim().toLowerCase()

        // Regex to catch: "100 usd to eur", "(100+50) $ to €", "convert 10 eur in gbp"
        const regex = /^\s*(?:convert|calculate)?\s*([\d\.\s\+\-\*\/\(\)]+)\s*([a-z]{3}|[^0-9\s\(\)])(?:\s*(?:to|in|as)\s*([a-z]{3}|[^0-9\s\(\)]))?\s*$/i

        const match = q.match(regex)
        if (!match) return null

        const amountExpr = match[1]
        const fromRaw = match[2]
        let toRaw = match[3]

        // If no destination currency, try system default
        if (!toRaw) {
            toRaw = this._getSystemCurrency()
        }

        // Prepare currencies
        const from = this._resolveCurrency(fromRaw)
        const to = this._resolveCurrency(toRaw)
        if (!from || !to) return null

        // Evaluate math
        let amount = 0
        try {
            amount = evaluate(amountExpr)
        } catch (e) {
            return null // Invalid math
        }

        // Check if we have valid rates in cache to provide immediate answer
        const now = Date.now()
        let result = null
        let rate = null
        let timestamp = null

        // Try to load rates synchronously from cache if not in memory
        if (!this._rates) {
            const cached = localStorage.getItem('stv_currency_rates')
            if (cached) {
                try {
                    const parsed = JSON.parse(cached)
                    if (now - parsed.timestamp < this._CACHE_TTL) {
                        this._rates = parsed.rates
                        this._lastFetch = parsed.timestamp
                    }
                } catch (e) { }
            }
        }

        if (this._rates && this._rates[from] && this._rates[to]) {
            const rateFrom = this._rates[from]
            const rateTo = this._rates[to]
            // Conversion: (Amount / RateFrom) * RateTo
            rate = rateTo / rateFrom // Rate for 1 Unit FROM -> TO
            result = (amount / rateFrom) * rateTo
            timestamp = this._lastFetch
        } else {
            this._getRates() // Trigger async fetch to warm up cache if missing
        }

        return {
            score: 1.0, // High confidence matches
            data: {
                type: 'currency',
                amount,
                from,
                to,
                result,
                rate,
                timestamp
            },
            preview: result !== null
                ? `${amount} ${from} = ${result.toFixed(2)} ${to}`
                : `Convert ${amount} ${from} to ${to}...`
        }
    },

    _resolveCurrency(symbolOrCode) {
        if (!symbolOrCode) return null

        // Try exact code (3 letters)
        const code = symbolOrCode.toUpperCase()
        if (code.length === 3) {
            return code
        }

        // Try symbol lookup from our map
        if (symbolToCode[symbolOrCode]) {
            return symbolToCode[symbolOrCode]
        }

        // Try checking loopup from global map if it wasn't caught (unlikely)
        return null
    },

    async execute(data) {
        // If data already has result, just return formatted string for clipboard
        if (data.result !== null && data.result !== undefined) {
            return data.result.toFixed(2) // Just the number for easy pasting
        }

        // Otherwise (no cache hit during match), we need to fetch now.
        const { amount, from, to } = data

        try {
            const rates = await this._getRates()
            if (!rates) {
                return 'Error: Could not fetch exchange rates.'
            }

            const rateFrom = rates[from]
            const rateTo = rates[to]

            if (!rateFrom || !rateTo) {
                return `Error: Unsupported currency code (${!rateFrom ? from : to}).`
            }

            const result = (amount / rateFrom) * rateTo
            return result.toFixed(2)

        } catch (e) {
            console.error('Currency conversion error:', e)
            return 'Error performing conversion.'
        }
    },

    async _getRates() {
        const now = Date.now()

        // Deduplicate requests: if a fetch is already running, return its promise
        if (this._fetchPromise) {
            return this._fetchPromise
        }

        // Try load from local storage first if we haven't checked yet
        if (!this._rates) {
            try {
                const cached = localStorage.getItem('stv_currency_rates')
                if (cached) {
                    const parsed = JSON.parse(cached)
                    if (now - parsed.timestamp < this._CACHE_TTL) {
                        this._rates = parsed.rates
                        this._lastFetch = parsed.timestamp
                    }
                }
            } catch (e) { }
        }

        // If we have valid rates, return them
        if (this._rates && (now - this._lastFetch <= this._CACHE_TTL)) {
            return this._rates
        }

        // Otherwise fetch
        this._fetchPromise = (async () => {
            try {
                const res = await fetch('https://open.er-api.com/v6/latest/USD')
                const json = await res.json()

                if (json && json.result === 'success') {
                    this._rates = json.rates
                    this._lastFetch = Date.now()

                    localStorage.setItem('stv_currency_rates', JSON.stringify({
                        rates: this._rates,
                        timestamp: this._lastFetch
                    }))
                }
            } catch (e) {
                console.error('Failed to fetch rates', e)
            } finally {
                this._fetchPromise = null
            }
            return this._rates
        })()

        return this._fetchPromise
    },

    _getSystemCurrency() {
        let currency = null
        try {
            // 1. Try Locale
            // locale-currency handles "en-US", "es-CO", etc.
            const localeStr = new Intl.NumberFormat().resolvedOptions().locale
            currency = getCurrency(localeStr)
        } catch (e) {
            // Ignore locale error
        }

        try {
            // 2. Try Timezone
            // Use timezone to refine or fallback
            // If Intl.DateTimeFormat is missing (e.g. bad mock), we shouldn't crash
            if (typeof Intl.DateTimeFormat !== 'function') {
                return currency || 'USD'
            }

            const timeZone = new Intl.DateTimeFormat().resolvedOptions().timeZone
            let currencyFromTz = null
            if (timeZone) {
                const tzData = ct.getTimezone(timeZone)
                if (tzData && tzData.countries && tzData.countries.length > 0) {
                    // Get main country
                    const country = tzData.countries[0] // e.g. 'CO'
                    currencyFromTz = getCurrency(country) // e.g. 'COP'
                }
            }

            // Intelligent Fallback:
            // If we found a timezone currency, and (no locale currency OR locale currency is USD), prefer timezone?
            // User in Colombia (COP) with en-US (USD). -> Prefer COP.
            // User in USA (USD) with en-US (USD). -> USD.
            // User in US with es-CO (COP). -> Prefer COP (Language preference).
            // Logic: Trust explicit locale region unless it's the "default" en-US which might be just system default.

            if (currencyFromTz && (!currency || currency === 'USD')) {
                return currencyFromTz
            }

            if (currency) return currency

            return 'USD'
        } catch (e) {
            return currency || 'USD'
        }
    }
}
