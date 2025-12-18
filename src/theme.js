export const themePresets = [
    {
        name: 'Tokyo Night',
        primary: '#7aa2f7',
        secondary: '#bb9af7',
        background: '#1a1b26',
        surface: '#24283b',
        text: '#c0caf5',
        is_custom: false
    },
    {
        name: 'Nord',
        primary: '#88c0d0',
        secondary: '#81a1c1',
        background: '#2e3440',
        surface: '#3b4252',
        text: '#d8dee9',
        is_custom: false
    },
    {
        name: 'Catppuccin Macchiato',
        primary: '#8aadf4',
        secondary: '#f5bde6',
        background: '#24273a',
        surface: '#363a4f',
        text: '#cad3f5',
        is_custom: false
    },
    {
        name: 'One Dark',
        primary: '#61afef',
        secondary: '#c678dd',
        background: '#282c34',
        surface: '#2c313a',
        text: '#abb2bf',
        is_custom: false
    },
    {
        name: 'Dracula',
        primary: '#bd93f9',
        secondary: '#ff79c6',
        background: '#282a36',
        surface: '#44475a',
        text: '#f8f8f2',
        is_custom: false
    },
    {
        name: 'Everforest',
        primary: '#a7c080',
        secondary: '#d699b6',
        background: '#2b3339',
        surface: '#323c41',
        text: '#d3c6aa',
        is_custom: false
    }
]

export function applyTheme(theme) {
    if (!theme) return

    const root = document.documentElement
    root.style.setProperty('--v-primary-base', theme.primary)
    root.style.setProperty('--v-secondary-base', theme.secondary)
    root.style.setProperty('--app-background', theme.background)
    root.style.setProperty('--app-surface', theme.surface)
    root.style.setProperty('--app-text', theme.text)

    // Custom CSS variables for components to use
    root.style.setProperty('--theme-primary', theme.primary)
    root.style.setProperty('--theme-secondary', theme.secondary)
    root.style.setProperty('--theme-background', theme.background)
    root.style.setProperty('--theme-surface', theme.surface)
    root.style.setProperty('--theme-text', theme.text)
}
