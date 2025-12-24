import { ref, watch, computed } from 'vue'
import { invoke, convertFileSrc } from '@tauri-apps/api/core'

export function useFilePreview(filePathRef) {
    const content = ref('')
    const metadata = ref(null)
    const isLoading = ref(false)
    const error = ref(null)
    const fileType = ref('unknown') // 'image', 'video', 'code', 'text', 'binary', 'unknown'

    const imageExtensions = ['png', 'jpg', 'jpeg', 'webp', 'gif', 'svg', 'bmp', 'ico']
    const videoExtensions = ['mp4', 'mkv', 'avi', 'mov', 'webm']
    const codeExtensions = [
        'js', 'ts', 'vue', 'py', 'rs', 'html', 'css', 'json', 'md', 'c', 'cpp', 'h', 'hpp',
        'java', 'go', 'rb', 'php', 'sh', 'yaml', 'yml', 'xml', 'toml', 'ini', 'sql'
    ]

    async function loadFile() {
        const path = filePathRef.value
        if (!path) {
            reset()
            return
        }

        isLoading.value = true
        error.value = null
        content.value = ''
        metadata.value = null

        try {
            // 1. Get Metadata
            try {
                metadata.value = await invoke('get_file_metadata', { path })
            } catch (e) {
                console.warn('Failed to get metadata', e)
                // Continue, as checking extension doesn't need metadata
            }

            // 2. Determine Type via Extension
            const ext = path.split('.').pop().toLowerCase()

            if (imageExtensions.includes(ext)) {
                fileType.value = 'image'
            } else if (videoExtensions.includes(ext)) {
                fileType.value = 'video'
            } else if (codeExtensions.includes(ext)) {
                fileType.value = 'code'
            } else {
                fileType.value = 'unknown' // Will check for text content
            }

            // 3. Load Content for Text/Code
            if (fileType.value === 'code' || fileType.value === 'unknown') {
                try {
                    const text = await invoke('read_file_preview', { path, maxBytes: 2048 })
                    content.value = text
                    if (fileType.value === 'unknown') {
                        fileType.value = 'text'
                    }
                } catch (e) {
                    if (e.includes('Binary')) {
                        fileType.value = 'binary'
                    } else if (fileType.value === 'code') {
                        // Failed to read code file? Keep as code but show error or empty
                        error.value = 'Could not read file content'
                    }
                }
            }

        } catch (e) {
            error.value = e.toString()
        } finally {
            isLoading.value = false
        }
    }

    function reset() {
        content.value = ''
        metadata.value = null
        isLoading.value = false
        error.value = null
        fileType.value = 'unknown'
    }

    watch(filePathRef, () => {
        loadFile()
    }, { immediate: true })

    const srcUrl = computed(() => {
        if (!filePathRef.value) return ''
        return convertFileSrc(filePathRef.value)
    })

    return {
        content,
        metadata,
        isLoading,
        error,
        fileType,
        srcUrl,
        srcUrl,
        reset,
        generateVideoThumbnail: async (path) => {
            return invoke('generate_video_thumbnail', { path })
        }
    }
}
