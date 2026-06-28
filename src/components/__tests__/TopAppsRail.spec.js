import { mount } from '@vue/test-utils'
import { describe, it, expect } from 'vitest'
import TopAppsRail from '../TopAppsRail.vue'

function makeApp(id, overrides = {}) {
    return {
        id,
        name: id.charAt(0).toUpperCase() + id.slice(1),
        exec: `/${id}`,
        source: 'desktop',
        icon: null,
        ...overrides,
    }
}

describe('TopAppsRail', () => {
    it('renders nothing when topApps is empty', () => {
        const wrapper = mount(TopAppsRail, {
            props: { topApps: [] },
        })
        expect(wrapper.find('[data-testid="top-apps-rail"]').exists()).toBe(false)
    })

    it('renders nothing when topApps is missing', () => {
        const wrapper = mount(TopAppsRail, {
            props: {},
        })
        expect(wrapper.find('[data-testid="top-apps-rail"]').exists()).toBe(false)
    })

    it('renders a button per app entry', () => {
        const apps = [
            makeApp('chrome'),
            makeApp('firefox'),
            makeApp('code'),
        ]
        const wrapper = mount(TopAppsRail, {
            props: { topApps: apps },
        })
        const items = wrapper.findAll('[data-testid^="top-app-"]')
        expect(items.length).toBe(3)
        expect(wrapper.text()).toContain('Chrome')
        expect(wrapper.text()).toContain('Firefox')
        expect(wrapper.text()).toContain('Code')
    })

    it('caps the rail at the max prop default of 8 entries', () => {
        const apps = Array.from({ length: 12 }, (_, i) => makeApp(`app-${i}`))
        const wrapper = mount(TopAppsRail, {
            props: { topApps: apps },
        })
        const items = wrapper.findAll('[data-testid^="top-app-"]')
        expect(items.length).toBe(8)
    })

    it('honors a custom max prop', () => {
        const apps = Array.from({ length: 10 }, (_, i) => makeApp(`app-${i}`))
        const wrapper = mount(TopAppsRail, {
            props: { topApps: apps, max: 3 },
        })
        const items = wrapper.findAll('[data-testid^="top-app-"]')
        expect(items.length).toBe(3)
    })

    it('preserves the caller-supplied order (decay ranking)', () => {
        const apps = [
            makeApp('alpha'),
            makeApp('beta'),
            makeApp('gamma'),
        ]
        const wrapper = mount(TopAppsRail, {
            props: { topApps: apps },
        })
        const items = wrapper.findAll('[data-testid^="top-app-"]')
        expect(items[0].attributes('data-app-id')).toBe('alpha')
        expect(items[1].attributes('data-app-id')).toBe('beta')
        expect(items[2].attributes('data-app-id')).toBe('gamma')
    })

    it('emits launch with the app payload when a tile is clicked', async () => {
        const apps = [makeApp('chrome'), makeApp('firefox')]
        const wrapper = mount(TopAppsRail, {
            props: { topApps: apps },
        })
        await wrapper.find('[data-testid="top-app-firefox"]').trigger('click')
        const events = wrapper.emitted('launch')
        expect(events).toBeTruthy()
        expect(events.length).toBe(1)
        expect(events[0][0].id).toBe('firefox')
    })

    it('marks the activeId tile with rail-item-active class', () => {
        const apps = [makeApp('chrome'), makeApp('firefox'), makeApp('code')]
        const wrapper = mount(TopAppsRail, {
            props: { topApps: apps, activeId: 'code' },
        })
        const active = wrapper.find('[data-testid="top-app-code"]')
        expect(active.classes()).toContain('rail-item-active')
        expect(wrapper.find('[data-testid="top-app-chrome"]').classes()).not.toContain('rail-item-active')
    })

    it('renders the source badge when source is provided', () => {
        const apps = [makeApp('chrome', { source: 'flatpak' })]
        const wrapper = mount(TopAppsRail, {
            props: { topApps: apps },
        })
        expect(wrapper.text()).toContain('Flatpak')
    })

    it('renders the fallback letter when no icon is provided', () => {
        const apps = [makeApp('chrome', { icon: null })]
        const wrapper = mount(TopAppsRail, {
            props: { topApps: apps },
        })
        expect(wrapper.find('.rail-icon-fallback').exists()).toBe(true)
        expect(wrapper.find('.rail-icon-fallback').text()).toBe('C')
    })

    it('uses the exec as the title attribute', () => {
        const apps = [makeApp('chrome', { exec: '/usr/bin/google-chrome' })]
        const wrapper = mount(TopAppsRail, {
            props: { topApps: apps },
        })
        const item = wrapper.find('[data-testid="top-app-chrome"]')
        expect(item.attributes('title')).toBe('/usr/bin/google-chrome')
    })
})