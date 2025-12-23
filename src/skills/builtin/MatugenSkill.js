import { open } from '@tauri-apps/plugin-dialog'
import { readFile } from '@tauri-apps/plugin-fs'
import { themeFromSourceColor, sourceColorFromImage, hexFromArgb } from '@material/material-color-utilities'
import { applyTheme } from '../../theme'
import { backend } from '../../adapters/tauriBackend'

export const MatugenSkill = {
    id: 'matugen-theme',
    name: 'Theme Generator',
    description: 'Generate colors from wallpaper/image',
    icon: '🎨',

    match(query) {
        if (!query) return null
        const q = query.toLowerCase()
        if (['theme', 'color', 'colors', 'matugen', 'wallpaper'].some(k => q.includes(k))) {
            return {
                score: 0.9,
                data: {},
                preview: 'Pick an image to generate theme colors'
            }
        }
        return null
    },

    async execute(data) {
        try {
            const file = await open({
                multiple: false,
                directory: false,
                filters: [{
                    name: 'Images',
                    extensions: ['png', 'jpg', 'jpeg', 'webp']
                }]
            })

            if (!file) return null // User cancelled

            // Read file as binary to avoid taint issues with asset:// protocol
            const contents = await readFile(file)
            const blob = new Blob([contents])
            const imageUrl = URL.createObjectURL(blob)
            const img = new Image()
            img.src = imageUrl
            await new Promise((resolve, reject) => {
                img.onload = resolve
                img.onerror = reject
            })

            const sourceColor = await sourceColorFromImage(img)
            const theme = themeFromSourceColor(sourceColor)

            // Cleanup
            URL.revokeObjectURL(imageUrl)

            // Extract dark scheme colors (StratosBar is dark mode)
            const scheme = theme.schemes.dark

            const newTheme = {
                name: 'custom-' + Date.now(),
                primary: hexFromArgb(scheme.primary),
                secondary: hexFromArgb(scheme.secondary),
                background: hexFromArgb(scheme.surface),
                surface: hexFromArgb(scheme.surfaceContainer),
                text: hexFromArgb(scheme.onSurface),
                is_custom: true,
                source_image: file.split('/').pop()
            }

            // Apply immediately
            applyTheme(newTheme)

            // Save to config
            try {
                const config = await backend.getConfig()
                config.theme = newTheme
                await backend.saveConfig(config)

                // Notify app to reload config
                window.dispatchEvent(new CustomEvent('reload-config'))
            } catch (e) {
                console.error('Failed to save theme config', e)
            }

            // 2. Run matugen for system-wide sync
            const safeFile = file.replace(/"/g, '\\"')
            const cmd = `matugen image "${safeFile}"`
            await backend.launchApp(cmd)

            return `Theme updated from ${file.split('/').pop()}`
        } catch (e) {
            console.error('Matugen error:', e)
            return 'Failed to update theme'
        }
    }
}
