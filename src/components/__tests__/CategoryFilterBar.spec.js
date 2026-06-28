import { mount } from '@vue/test-utils'
import { describe, it, expect } from 'vitest'
import CategoryFilterBar from '../CategoryFilterBar.vue'

const DEFAULT_OPTIONS = [
    { id: 'development', label: 'Development' },
    { id: 'internet', label: 'Internet' },
    { id: 'multimedia', label: 'Multimedia' },
]

describe('CategoryFilterBar', () => {
    it('renders nothing when options is empty', () => {
        const wrapper = mount(CategoryFilterBar, {
            props: { options: [] },
        })
        expect(wrapper.find('[data-testid="category-filter-bar"]').exists()).toBe(false)
    })

    it('renders an All chip plus one chip per provided option', () => {
        const wrapper = mount(CategoryFilterBar, {
            props: { options: DEFAULT_OPTIONS },
        })
        const chips = wrapper.findAll('[data-testid^="category-chip-"]')
        expect(chips.length).toBe(DEFAULT_OPTIONS.length + 1)
        expect(wrapper.find('[data-testid="category-chip-all"]').exists()).toBe(true)
        expect(wrapper.find('[data-testid="category-chip-development"]').exists()).toBe(true)
        expect(wrapper.find('[data-testid="category-chip-internet"]').exists()).toBe(true)
        expect(wrapper.find('[data-testid="category-chip-multimedia"]').exists()).toBe(true)
    })

    it('labels each chip using the option label', () => {
        const wrapper = mount(CategoryFilterBar, {
            props: { options: DEFAULT_OPTIONS },
        })
        expect(wrapper.find('[data-testid="category-chip-development"]').text()).toContain('Development')
        expect(wrapper.find('[data-testid="category-chip-internet"]').text()).toContain('Internet')
        expect(wrapper.find('[data-testid="category-chip-multimedia"]').text()).toContain('Multimedia')
    })

    it('marks the All chip as active when active is empty', () => {
        const wrapper = mount(CategoryFilterBar, {
            props: { options: DEFAULT_OPTIONS, active: '' },
        })
        const all = wrapper.find('[data-testid="category-chip-all"]')
        expect(all.classes()).toContain('category-chip-active')
    })

    it('marks the active chip and clears All when active matches an option id', () => {
        const wrapper = mount(CategoryFilterBar, {
            props: { options: DEFAULT_OPTIONS, active: 'development' },
        })
        const dev = wrapper.find('[data-testid="category-chip-development"]')
        expect(dev.classes()).toContain('category-chip-active')
        expect(wrapper.find('[data-testid="category-chip-all"]').classes()).not.toContain('category-chip-active')
    })

    it('is case-insensitive when matching the active id', () => {
        const wrapper = mount(CategoryFilterBar, {
            props: { options: DEFAULT_OPTIONS, active: 'Development' },
        })
        const dev = wrapper.find('[data-testid="category-chip-development"]')
        expect(dev.classes()).toContain('category-chip-active')
    })

    it('emits select with the option id when a chip is clicked', async () => {
        const wrapper = mount(CategoryFilterBar, {
            props: { options: DEFAULT_OPTIONS, active: '' },
        })
        await wrapper.find('[data-testid="category-chip-internet"]').trigger('click')
        const events = wrapper.emitted('select')
        expect(events).toBeTruthy()
        expect(events.length).toBe(1)
        expect(events[0][0]).toBe('internet')
    })

    it('emits an empty string when the currently-active chip is clicked again (clears the filter)', async () => {
        const wrapper = mount(CategoryFilterBar, {
            props: { options: DEFAULT_OPTIONS, active: 'development' },
        })
        await wrapper.find('[data-testid="category-chip-development"]').trigger('click')
        const events = wrapper.emitted('select')
        expect(events[events.length - 1][0]).toBe('')
    })

    it('emits an empty string when the All chip is clicked while already in All state', async () => {
        const wrapper = mount(CategoryFilterBar, {
            props: { options: DEFAULT_OPTIONS, active: '' },
        })
        await wrapper.find('[data-testid="category-chip-all"]').trigger('click')
        const events = wrapper.emitted('select')
        expect(events[events.length - 1][0]).toBe('')
    })

    it('emits the new id when clicking All from another active state', async () => {
        const wrapper = mount(CategoryFilterBar, {
            props: { options: DEFAULT_OPTIONS, active: 'internet' },
        })
        await wrapper.find('[data-testid="category-chip-all"]').trigger('click')
        const events = wrapper.emitted('select')
        expect(events[events.length - 1][0]).toBe('')
    })

    it('renders the count next to the chip label when option has count', () => {
        const wrapper = mount(CategoryFilterBar, {
            props: {
                options: [
                    { id: 'development', label: 'Development', count: 12 },
                    { id: 'internet', label: 'Internet' },
                ],
            },
        })
        expect(wrapper.find('[data-testid="category-chip-development"]').text()).toContain('12')
        expect(wrapper.find('[data-testid="category-chip-internet"]').text()).not.toContain('12')
    })

    it('does not render any chip for an option missing an id', () => {
        const wrapper = mount(CategoryFilterBar, {
            props: {
                options: [
                    { id: 'development', label: 'Development' },
                    { label: 'Ghost' },
                ],
            },
        })
        expect(wrapper.find('[data-testid="category-chip-ghost"]').exists()).toBe(false)
    })
})