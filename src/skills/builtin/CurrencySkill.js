import { evaluate } from 'mathjs'

export const CurrencySkill = {
    id: 'builtin-currency',
    name: 'Currency Converter',
    description: 'Convert between currencies (e.g. 10 usd to eur)',
    icon: '💱',

    // Cache config
    _rates: null,
    _lastFetch: 0,
    _CACHE_TTL: 24 * 60 * 60 * 1000, // 24 hours

    // Supported symbols map for convenience
    // This is a small subset; the API supports many more 3-letter codes.
    _symbols: {
        '$': 'USD',
        '€': 'EUR',
        '£': 'GBP',
        '¥': 'JPY',
        '₹': 'INR',
        '₽': 'RUB',
        '₩': 'KRW',
        '₿': 'BTC'
    },

    match(query) {
        if (!query) return null

        const q = query.trim().toLowerCase()

        // Regex to catch: "100 usd to eur", "(100+50) $ to €", "convert 10 eur in gbp"
        // Improved regex to capture math expression as group 1
        // Group 1: expression (numbers, operators, parens)
        // Group 2: from currency
        // Group 3: to currency
        const regex = /^\s*(?:convert|calculate)?\s*([\d\.\s\+\-\*\/\(\)]+)\s*([a-z]{3}|[$€£¥₹₽₩₿])\s*(?:to|in|as)\s*([a-z]{3}|[$€£¥₹₽₩₿])\s*$/i

        const match = q.match(regex)
        if (!match) return null

        const amountExpr = match[1]
        const fromRaw = match[2]
        const toRaw = match[3]

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

        // We need rates to calculate the preview. 
        // match() is synchronous but we might have cached rates. 
        // If we don't have rates, we can't show the calculation in preview immediately 
        // unless we make match async or have separate 'resolve' phase.
        // For now, we rely on the fact that we might have rates or we return a "loading" state object.
        // StratosBar architecture seems to assume match is sync.

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
            // Trigger a background fetch for next time if needed? 
            // Or rely on the UI component to fetch if 'rate' is missing.
            // We'll return match found, but with null result.
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
        // Check if it's a known symbol
        if (this._symbols[symbolOrCode]) {
            return this._symbols[symbolOrCode]
        }
        // Otherwise assume it's a 3-letter code
        const code = symbolOrCode.toUpperCase()
        if (code.length === 3) {
            return code
        }
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

            // The API returns rates relative to base (USD usually for the link we use, or we can check base)
            // rates[code] = value. 
            // open.er-api.com/v6/latest/USD returns rates where 1 USD = X Currency

            const rateFrom = rates[from]
            const rateTo = rates[to]

            if (!rateFrom || !rateTo) {
                return `Error: Unsupported currency code (${!rateFrom ? from : to}).`
            }

            // Convert: (Amount / RateFrom) * RateTo
            // Example: 100 EUR to JPY. Base USD.
            // 100 EUR = (100 / RateEUR_per_USD) USD
            // Result JPY = USD_Amount * RateJPY_per_USD

            const result = (amount / rateFrom) * rateTo
            return result.toFixed(2)

        } catch (e) {
            console.error('Currency conversion error:', e)
            return 'Error performing conversion.'
        }
    },

    async _getRates() {
        const now = Date.now()

        // Try load from local storage first
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
            } catch (e) {
                // ignore json error
            }
        }

        // If still no rates or expired, fetch
        if (!this._rates || (now - this._lastFetch > this._CACHE_TTL)) {
            try {
                // Using open.er-api.com - Free, no key required, updates daily
                const res = await fetch('https://open.er-api.com/v6/latest/USD')
                const json = await res.json()

                if (json && json.result === 'success') {
                    this._rates = json.rates
                    this._lastFetch = now

                    // Save to cache
                    localStorage.setItem('stv_currency_rates', JSON.stringify({
                        rates: this._rates,
                        timestamp: this._lastFetch
                    }))
                }
            } catch (e) {
                console.error('Failed to fetch rates', e)
                // If fetch failed but we have old rates, execute with old rates?
                // For now, just return what we have (might be null)
            }
        }

        return this._rates
    }
}
