<template>
  <div v-if="hasEntries" class="top-apps-rail" data-testid="top-apps-rail">
    <div class="section-header d-flex align-center justify-space-between">
      <span>TOP APPS</span>
      <span class="text-dimmer text-caption">Most used</span>
    </div>
    <div class="rail-track custom-scrollbar" data-testid="top-apps-track">
      <button
        v-for="app in limitedApps"
        :key="app.id"
        type="button"
        class="rail-item interactive"
        :class="{ 'rail-item-active': activeId === app.id }"
        :data-testid="`top-app-${app.id}`"
        :data-app-id="app.id"
        :title="app.exec"
        @click="$emit('launch', app)"
      >
        <div class="rail-icon">
          <img v-if="app.icon" :src="iconSrc(app.icon)" width="32" height="32" alt="" />
          <span v-else class="rail-icon-fallback">{{ fallbackLetter(app.name) }}</span>
        </div>
        <div class="rail-name">{{ app.name }}</div>
        <span v-if="app.source" class="source-badge" :class="`source-${String(app.source).toLowerCase()}`">
          {{ sourceLabel(app.source) }}
        </span>
      </button>
    </div>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'

const props = defineProps({
    topApps: {
        type: Array,
        default: () => [],
    },
    activeId: {
        type: String,
        default: null,
    },
    max: {
        type: Number,
        default: 8,
        validator: (value) => Number.isInteger(value) && value > 0,
    },
})

defineEmits(['launch'])

const hasEntries = computed(() => Array.isArray(props.topApps) && props.topApps.length > 0)

const limitedApps = computed(() => {
    if (!Array.isArray(props.topApps)) return []
    return props.topApps.slice(0, props.max)
})

function iconSrc(icon) {
    if (!icon) return ''
    try {
        return convertFileSrc(icon)
    } catch (e) {
        return icon
    }
}

function fallbackLetter(name) {
    if (!name) return '·'
    const first = String(name).trim().charAt(0)
    return first ? first.toUpperCase() : '·'
}

function sourceLabel(source) {
    if (!source) return ''
    const normalized = String(source).toLowerCase()
    return normalized.charAt(0).toUpperCase() + normalized.slice(1)
}
</script>

<style scoped>
.top-apps-rail {
    margin-bottom: var(--space-4);
}

.rail-track {
    display: flex;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-4) var(--space-3);
    overflow-x: auto;
    scrollbar-width: thin;
    scroll-snap-type: x proximity;
}

.rail-track::-webkit-scrollbar {
    height: 6px;
}

.rail-track::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.08);
    border-radius: var(--radius-full);
}

.rail-item {
    flex: 0 0 auto;
    min-width: 96px;
    max-width: 120px;
    padding: var(--space-3) var(--space-2);
    border-radius: var(--radius-lg);
    border: 1px solid var(--theme-border);
    background: rgba(255, 255, 255, 0.03);
    color: var(--theme-text);
    cursor: pointer;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-2);
    scroll-snap-align: start;
    transition: all var(--duration-fast) var(--ease-out);
    text-align: center;
}

.rail-item:hover {
    background: rgba(255, 255, 255, 0.06);
    border-color: var(--theme-border-hover);
    transform: translateY(-1px);
}

.rail-item-active {
    background: rgba(122, 162, 247, 0.15);
    border-color: rgba(122, 162, 247, 0.45);
    box-shadow: inset 0 0 0 1px rgba(122, 162, 247, 0.2);
}

.rail-icon {
    width: 40px;
    height: 40px;
    border-radius: var(--radius-md);
    background: rgba(255, 255, 255, 0.05);
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    flex-shrink: 0;
}

.rail-icon img {
    object-fit: contain;
}

.rail-icon-fallback {
    font-size: 1rem;
    font-weight: var(--font-weight-semibold);
    color: var(--theme-primary, #7aa2f7);
}

.rail-name {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    color: var(--theme-text);
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.source-badge {
    font-size: 9px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 1px 5px;
    border-radius: var(--radius-sm);
    background: rgba(255, 255, 255, 0.05);
    color: var(--theme-text-dim);
    border: 1px solid rgba(255, 255, 255, 0.08);
}

.source-badge.source-flatpak {
    background: rgba(122, 162, 247, 0.15);
    color: #7aa2f7;
    border-color: rgba(122, 162, 247, 0.3);
}

.source-badge.source-snap {
    background: rgba(247, 122, 162, 0.15);
    color: #f77aa2;
    border-color: rgba(247, 122, 162, 0.3);
}

.source-badge.source-appimage {
    background: rgba(247, 200, 122, 0.15);
    color: #f7c87a;
    border-color: rgba(247, 200, 122, 0.3);
}

.source-badge.source-nix {
    background: rgba(122, 247, 162, 0.15);
    color: #7af7a2;
    border-color: rgba(122, 247, 162, 0.3);
}

.section-header {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--theme-text-dimmer);
    letter-spacing: 0.05em;
    padding: var(--space-2) var(--space-4);
    margin-bottom: var(--space-2);
}

.text-caption {
    font-size: var(--font-size-xs);
}

.text-dimmer {
    color: var(--theme-text-dimmer);
}
</style>