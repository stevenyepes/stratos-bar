import { mount } from '@vue/test-utils'
import { describe, it, expect, vi } from 'vitest'

// Matches the shape Tauri's real `convertFileSrc` produces on Linux, so the
// assertions below exercise the same string the webview would request.
vi.mock('@tauri-apps/api/core', () => ({
    convertFileSrc: (path: string, protocol: string) =>
        `${protocol}://localhost/${encodeURIComponent(path)}`,
}))

import CurrencyResult from '../CurrencyResult.vue'
import { createVuetify } from 'vuetify'
import * as components from 'vuetify/components'
import * as directives from 'vuetify/directives'

const vuetify = createVuetify({
    components,
    directives,
})

function mountWith(data: Record<string, unknown>) {
    return mount(CurrencyResult, {
        global: { plugins: [vuetify] },
        props: {
            data: { result: 100, rate: 1.2, amount: 83.33, timestamp: Date.now(), ...data },
        },
    })
}

describe('CurrencyResult', () => {
    it('renders properties properly', () => {
        const wrapper = mount(CurrencyResult, {
            global: {
                plugins: [vuetify],
            },
            props: {
                data: {
                    result: 100.00,
                    to: "USD",
                    rate: 1.2,
                    timestamp: Date.now(), // Use current time to avoid "just now" issues if needed, or specific
                    from: "EUR",
                    amount: 83.33
                }
            }
        })

        // Check for main result text
        expect(wrapper.text()).toContain('100.00')
        expect(wrapper.text()).toContain('USD')

        // Check for rate text (logic from component: 1 FROM = RATE TO)
        // 1 EUR = 1.2000 USD
        expect(wrapper.text()).toContain('1 EUR = 1.2000 USD')
    })

    it('serves both flags from the local stratos-icon protocol, not a remote CDN', () => {
        const wrapper = mountWith({ from: 'EUR', to: 'JPY' })
        const srcs = wrapper.findAll('img.currency-flag').map((img) => img.attributes('src'))

        expect(srcs).toEqual([
            'stratos-icon://localhost/flags%2Feu.svg',
            'stratos-icon://localhost/flags%2Fjp.svg',
        ])
        srcs.forEach((src) => expect(src).not.toContain('http'))
    })

    it('renders the local xx placeholder for a currency with no mapped flag', () => {
        const wrapper = mountWith({ from: 'XYZ', to: 'USD' })
        const srcs = wrapper.findAll('img.currency-flag').map((img) => img.attributes('src'))

        expect(srcs[0]).toBe('stratos-icon://localhost/flags%2Fxx.svg')
        expect(srcs[1]).toBe('stratos-icon://localhost/flags%2Fus.svg')
    })

    it('falls back to the local placeholder when a flag fails to load', async () => {
        const wrapper = mountWith({ from: 'EUR', to: 'USD' })
        const img = wrapper.findAll('img.currency-flag')[0]

        await img.trigger('error')

        expect((img.element as HTMLImageElement).src).toBe(
            'stratos-icon://localhost/flags%2Fxx.svg'
        )
    })

    it('does not retry when the placeholder itself fails to load', async () => {
        const wrapper = mountWith({ from: 'XYZ', to: 'USD' })
        const img = wrapper.findAll('img.currency-flag')[0]
        const element = img.element as HTMLImageElement
        const setSrc = vi.spyOn(element, 'src', 'set')

        await img.trigger('error')

        expect(setSrc).not.toHaveBeenCalled()
    })
})
