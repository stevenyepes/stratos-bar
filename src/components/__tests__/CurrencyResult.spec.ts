import { mount } from '@vue/test-utils'
import { describe, it, expect } from 'vitest'
import CurrencyResult from '../CurrencyResult.vue'
import { createVuetify } from 'vuetify'
import * as components from 'vuetify/components'
import * as directives from 'vuetify/directives'

const vuetify = createVuetify({
    components,
    directives,
})

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
})
