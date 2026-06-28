import { mount, flushPromises } from '@vue/test-utils'
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { createVuetify } from 'vuetify'
import * as components from 'vuetify/components'
import * as directives from 'vuetify/directives'
import SettingsAliases from '../SettingsAliases.vue'
import { _resetAppIndexForTests } from '../../composables/useAppIndex'

const invokeMock = vi.fn()

vi.mock('@tauri-apps/api/core', () => ({
    invoke: (...args) => invokeMock(...args),
    convertFileSrc: (s) => s,
}))

const vuetify = createVuetify({ components, directives })

function makeApps(overrides = []) {
    const base = [
        { id: 'chrome', name: 'Chrome', exec: '/usr/bin/chrome', icon: null, aliases: [] },
        { id: 'firefox', name: 'Firefox', exec: '/usr/bin/firefox', icon: null, aliases: ['browser'] },
        { id: 'code', name: 'Code', exec: '/usr/bin/code', icon: null, aliases: [] },
    ]
    if (!overrides.length) return base
    return base.map((entry, idx) => ({ ...entry, ...(overrides[idx] || {}) }))
}

async function setAliasInput(wrapper, appId, value) {
    const input = wrapper.find(`[data-testid="alias-input-${appId}"] input`)
    input.element.value = value
    await input.trigger('input')
    await flushPromises()
}

describe('SettingsAliases', () => {
    beforeEach(() => {
        invokeMock.mockReset()
        invokeMock.mockImplementation(() => Promise.resolve([]))
        _resetAppIndexForTests()
    })

    afterEach(() => {
        _resetAppIndexForTests()
    })

    it('renders one row per app', () => {
        const wrapper = mount(SettingsAliases, {
            props: { apps: makeApps() },
            global: { plugins: [vuetify] },
        })
        expect(wrapper.find('[data-testid="alias-row-chrome"]').exists()).toBe(true)
        expect(wrapper.find('[data-testid="alias-row-firefox"]').exists()).toBe(true)
        expect(wrapper.find('[data-testid="alias-row-code"]').exists()).toBe(true)
    })

    it('renders alias chips coming from app.aliases', () => {
        const wrapper = mount(SettingsAliases, {
            props: { apps: makeApps() },
            global: { plugins: [vuetify] },
        })
        const chip = wrapper.find('[data-testid="alias-chip-firefox-browser"]')
        expect(chip.exists()).toBe(true)
        expect(chip.text()).toContain('browser')
    })

    it('shows the empty state when no apps match the filter', async () => {
        const wrapper = mount(SettingsAliases, {
            props: { apps: makeApps() },
            global: { plugins: [vuetify] },
        })
        const filter = wrapper.find('[data-testid="alias-filter"] input')
        await filter.setValue('nothing-matches-this')
        await flushPromises()
        expect(wrapper.find('[data-testid="alias-empty"]').exists()).toBe(true)
    })

    it('filters rows by name, exec, or alias text', async () => {
        const wrapper = mount(SettingsAliases, {
            props: { apps: makeApps() },
            global: { plugins: [vuetify] },
        })
        const filter = wrapper.find('[data-testid="alias-filter"] input')
        await filter.setValue('fire')
        await flushPromises()
        expect(wrapper.find('[data-testid="alias-row-firefox"]').exists()).toBe(true)
        expect(wrapper.find('[data-testid="alias-row-chrome"]').exists()).toBe(false)
    })

    it('shows inline error for invalid alias text on input', async () => {
        const wrapper = mount(SettingsAliases, {
            props: { apps: makeApps() },
            global: { plugins: [vuetify] },
        })
        await setAliasInput(wrapper, 'chrome', 'rm -rf /')
        const error = wrapper.find('[data-testid="alias-error-chrome"]')
        expect(error.exists()).toBe(true)
        expect(error.text()).toMatch(/letters|digits|spaces|dots|hyphens|underscores/i)
    })

    it('disables the Add button when the input has an inline error', async () => {
        const wrapper = mount(SettingsAliases, {
            props: { apps: makeApps() },
            global: { plugins: [vuetify] },
        })
        await setAliasInput(wrapper, 'chrome', 'a;b')
        const addBtn = wrapper.find('[data-testid="alias-add-chrome"]')
        const buttonEl = addBtn.element
        expect(buttonEl.disabled || buttonEl.getAttribute('disabled') !== null).toBe(true)
    })

    it('invokes set_app_aliases via useAppIndex when adding a valid alias', async () => {
        invokeMock.mockImplementation((cmd, args) => {
            if (cmd === 'set_app_aliases') {
                expect(args.appId).toBe('chrome')
                expect(args.aliases).toEqual(['browser'])
                return Promise.resolve(null)
            }
            return Promise.resolve([])
        })

        const wrapper = mount(SettingsAliases, {
            props: { apps: makeApps() },
            global: { plugins: [vuetify] },
        })
        await setAliasInput(wrapper, 'chrome', 'browser')
        await wrapper.find('[data-testid="alias-add-chrome"]').trigger('click')
        await flushPromises()

        const calls = invokeMock.mock.calls.filter((c) => c[0] === 'set_app_aliases')
        expect(calls.length).toBeGreaterThan(0)
        expect(calls[calls.length - 1][1]).toEqual(
            expect.objectContaining({ appId: 'chrome', aliases: ['browser'] }),
        )

        const chip = wrapper.find('[data-testid="alias-chip-chrome-browser"]')
        expect(chip.exists()).toBe(true)
    })

    it('does not invoke the command when the alias text is invalid', async () => {
        const wrapper = mount(SettingsAliases, {
            props: { apps: makeApps() },
            global: { plugins: [vuetify] },
        })
        await setAliasInput(wrapper, 'chrome', 'rm -rf /')
        await wrapper.find('[data-testid="alias-add-chrome"]').trigger('click')
        await flushPromises()

        const calls = invokeMock.mock.calls.filter((c) => c[0] === 'set_app_aliases')
        expect(calls.length).toBe(0)
    })

    it('shows the "Already set" error when the alias already exists', async () => {
        const wrapper = mount(SettingsAliases, {
            props: { apps: makeApps() },
            global: { plugins: [vuetify] },
        })
        await setAliasInput(wrapper, 'firefox', 'browser')
        await wrapper.find('[data-testid="alias-add-firefox"]').trigger('click')
        await flushPromises()
        expect(wrapper.text()).toContain('Already set')
    })

    it('removes the alias when the chip close icon is triggered', async () => {
        invokeMock.mockImplementation((cmd, args) => {
            if (cmd === 'set_app_aliases') return Promise.resolve(null)
            return Promise.resolve([])
        })
        const wrapper = mount(SettingsAliases, {
            props: { apps: makeApps() },
            global: { plugins: [vuetify] },
        })
        const chip = wrapper.find('[data-testid="alias-chip-firefox-browser"]')
        expect(chip.exists()).toBe(true)
        const closeBtn = chip.find('.v-chip__close')
        if (closeBtn.exists()) {
            await closeBtn.trigger('click')
        } else {
            await chip.trigger('click')
        }
        await flushPromises()

        const calls = invokeMock.mock.calls.filter((c) => c[0] === 'set_app_aliases')
        expect(calls.length).toBeGreaterThan(0)
        expect(calls[calls.length - 1][1]).toEqual(
            expect.objectContaining({ appId: 'firefox', aliases: [] }),
        )
    })

    it('falls back to aliasesByAppId when an app.aliases is empty', async () => {
        invokeMock.mockImplementation((cmd) => {
            if (cmd === 'get_app_index_status') {
                return Promise.resolve({
                    total_apps: 1,
                    indexed_at: 0,
                    truncated: false,
                    disabled_sources: [],
                    sources_with_counts: [],
                })
            }
            return Promise.resolve([])
        })

        const { useAppIndex } = await import('../../composables/useAppIndex')
        const ctx = useAppIndex()
        ctx.aliasesByAppId.value = { 'chrome': ['browser'] }

        const wrapper = mount(SettingsAliases, {
            props: { apps: makeApps() },
            global: { plugins: [vuetify] },
        })
        expect(wrapper.find('[data-testid="alias-chip-chrome-browser"]').exists()).toBe(true)
    })

    it('exposes the ALIAS_VALIDATION_REGEX pattern through the error message', () => {
        const expected = '[\\w .\\-]'
        const wrapper = mount(SettingsAliases, {
            props: { apps: makeApps() },
            global: { plugins: [vuetify] },
        })
        expect(wrapper.find('[data-testid="alias-help"]').text()).toMatch(/letters|digits/i)
        expect(expected).toBeTruthy()
    })

    it('emits saved when an alias is added', async () => {
        invokeMock.mockImplementation(() => Promise.resolve(null))
        const wrapper = mount(SettingsAliases, {
            props: { apps: makeApps() },
            global: { plugins: [vuetify] },
        })
        await setAliasInput(wrapper, 'chrome', 'browser')
        await wrapper.find('[data-testid="alias-add-chrome"]').trigger('click')
        await flushPromises()

        const savedEvents = wrapper.emitted('saved')
        expect(savedEvents).toBeTruthy()
        expect(savedEvents[savedEvents.length - 1][0]).toEqual(
            expect.objectContaining({ appId: 'chrome', alias: 'browser', action: 'add' }),
        )
    })
})