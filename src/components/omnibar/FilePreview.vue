<template>
  <div class="file-preview-container h-100 d-flex flex-column">
    <!-- Breadcrumb -->
    <div class="preview-header font-mono text-xs text-dimmer truncate px-4 py-3 border-b">
      {{ filePath }}
    </div>

    <!-- Content Area -->
    <div class="preview-content flex-grow-1 relative overflow-hidden d-flex align-center justify-center bg-black-dim">
      
      <!-- Loading -->
      <div v-if="isLoading" class="d-flex flex-column align-center">
        <v-progress-circular indeterminate color="primary" size="32"></v-progress-circular>
      </div>

      <!-- Error -->
      <div v-else-if="error" class="text-center pa-4 text-error">
        <v-icon icon="mdi-alert-circle" size="large" class="mb-2"></v-icon>
        <div class="text-caption">Preview unavailable</div>
        <div class="text-xs text-dimmer mt-1">{{ error }}</div>
      </div>

      <!-- IMAGE -->
      <img 
        v-else-if="fileType === 'image'" 
        :src="srcUrl" 
        class="preview-media"
      />

      <!-- VIDEO -->
      <div 
        v-else-if="fileType === 'video'" 
        class="h-100 w-100 d-flex align-center justify-center relative"
      >
        <video
          v-show="!videoError"
          ref="videoRef"
          :src="srcUrl"
          class="preview-media"
          muted
          loop
          playsinline
          @loadedmetadata="onVideoLoaded"
          @error="onVideoError"
          @mouseenter="playVideo"
          @mouseleave="pauseVideo"
        ></video>
        
        <div v-if="videoError" class="text-center pa-4 text-warning">
             <v-icon icon="mdi-video-off" size="large" class="mb-2"></v-icon>
             <div class="text-caption">Playback failed</div>
             <div class="text-xs text-dimmer mt-1">Missing codecs or invalid format</div>
        </div>
      </div>

      <!-- CODE / TEXT -->
      <div v-else-if="fileType === 'code' || fileType === 'text'" class="preview-code-container custom-scrollbar w-100 h-100">
        <pre><code ref="codeBlock" :class="languageClass">{{ content }}</code></pre>
      </div>

      <!-- UNKNOWN / BINARY -->
      <div v-else class="text-center pa-6 opacity-50">
         <v-icon icon="mdi-file-question" size="64" class="mb-4"></v-icon>
         <div class="text-h6">
           {{ fileType === 'binary' ? 'Binary File' : 'Unknown File Type' }}
         </div>
         <div v-if="metadata" class="text-caption mt-2 text-dimmer">
            Modified: {{ formatDate(metadata.created) }}
         </div>
      </div>

    </div>

    <!-- Footer Metadata -->
    <div v-if="metadata || videoMeta" class="preview-footer px-4 py-2 border-t text-xs text-dim d-flex justify-space-between align-center">
      <div class="d-flex gap-4">
        <span v-if="videoMeta">
           {{ videoMeta.width }}x{{ videoMeta.height }} | {{ formatDuration(videoMeta.duration) }}
        </span>
        <span v-if="metadata">Size: {{ formatSize(metadata.size) }}</span>
      </div>
      <!-- Action Hints -->
      <div class="action-hints d-flex gap-3 text-dimmer">
        <span><kbd>↵</kbd> Open</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { toRef, ref, watch, nextTick, computed } from 'vue'
import { useFilePreview } from '../../composables/useFilePreview'
import hljs from 'highlight.js'
import 'highlight.js/styles/atom-one-dark.css' // Or another dark theme

const props = defineProps({
  filePath: {
    type: String,
    required: true
  }
})

const filePathRef = toRef(props, 'filePath')
const { content, metadata, isLoading, error, fileType, srcUrl } = useFilePreview(filePathRef)

// Video Logic
const videoRef = ref(null)
const videoMeta = ref(null)
const videoError = ref(false)

function onVideoLoaded(e) {
  const v = e.target
  videoMeta.value = {
    width: v.videoWidth,
    height: v.videoHeight,
    duration: v.duration
  }
}

function onVideoError(e) {
    console.warn("Video playback error:", e)
    videoError.value = true
}

function playVideo() {
  if(videoRef.value && !videoError.value) videoRef.value.play().catch(() => {})
}

function pauseVideo() {
  if(videoRef.value) videoRef.value.pause()
}

// Reset video meta when path changes
watch(filePathRef, () => {
    videoMeta.value = null
    videoError.value = false
})


// Code Highlight Logic
const codeBlock = ref(null)

const languageClass = computed(() => {
    if (!props.filePath) return ''
    const ext = props.filePath.split('.').pop().toLowerCase()
    return `language-${ext}` // highlight.js auto-inference usually works with this or alias
})

watch(content, () => {
  if ((fileType.value === 'code' || fileType.value === 'text') && content.value) {
    nextTick(() => {
        if (codeBlock.value) {
            delete codeBlock.value.dataset.highlighted // Force re-highlight if needed? 
            hljs.highlightElement(codeBlock.value)
        }
    })
  }
})

// Formatters
function formatSize(bytes) {
    if (bytes === 0) return '0 B'
    const k = 1024
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
    const i = Math.floor(Math.log(bytes) / Math.log(k))
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i]
}

function formatDate(timestamp) {
    if (!timestamp) return 'Unknown'
    return new Date(timestamp * 1000).toLocaleDateString(undefined, {
        year: 'numeric', month: 'short', day: 'numeric'
    })
}

function formatDuration(seconds) {
    if (!seconds) return '00:00'
    const m = Math.floor(seconds / 60)
    const s = Math.floor(seconds % 60)
    return `${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`
}

</script>

<style scoped>
.preview-media {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
}

.bg-black-dim {
    background: rgba(0, 0, 0, 0.3);
}

.border-b { border-bottom: 1px solid var(--theme-border); }
.border-t { border-top: 1px solid var(--theme-border); }

.preview-code-container {
    padding: 1rem;
    font-size: 12px;
    line-height: 1.5;
    background: #1e1e1e; /* Match hljs theme usually */
}

/* Ensure pre/code wrapping */
pre {
    margin: 0;
    white-space: pre-wrap;
    word-break: break-all;
}

kbd {
    background: rgba(255,255,255,0.1);
    border-radius: 4px;
    padding: 2px 4px;
    font-family: inherit;
    font-size: 10px;
}

.gap-4 { gap: 1rem; }
.gap-3 { gap: 0.75rem; }
</style>
