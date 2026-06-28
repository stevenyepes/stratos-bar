<template>
  <div
    v-if="visibleOptions.length > 0"
    class="category-filter-bar custom-scrollbar"
    data-testid="category-filter-bar"
  >
    <button
      type="button"
      class="category-chip interactive"
      :class="{ 'category-chip-active': !normalizedActive }"
      :data-testid="'category-chip-all'"
      :data-category="''"
      @click="handleClick('')"
    >
      <span class="category-chip-label">All</span>
    </button>
    <button
      v-for="option in visibleOptions"
      :key="option.id"
      type="button"
      class="category-chip interactive"
      :class="{ 'category-chip-active': normalizedActive === option.id }"
      :data-testid="`category-chip-${option.id}`"
      :data-category="option.id"
      @click="handleClick(option.id)"
    >
      <span class="category-chip-label">{{ option.label }}</span>
      <span v-if="typeof option.count === 'number'" class="category-chip-count text-dimmer">
        {{ option.count }}
      </span>
    </button>
  </div>
</template>

<script setup>
import { computed } from 'vue'

const props = defineProps({
    options: {
        type: Array,
        default: () => [],
    },
    active: {
        type: String,
        default: '',
    },
})

const emit = defineEmits(['select'])

const normalizedActive = computed(() => {
    if (!props.active) return ''
    return String(props.active).trim().toLowerCase()
})

const visibleOptions = computed(() => {
    if (!Array.isArray(props.options)) return []
    return props.options.filter((option) => option && option.id)
})

function handleClick(optionId) {
    const next = optionId || ''
    if (normalizedActive.value === next.toLowerCase()) {
        emit('select', '')
        return
    }
    emit('select', next)
}
</script>

<style scoped>
.category-filter-bar {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-4);
    overflow-x: auto;
    scrollbar-width: none;
    flex-shrink: 0;
}

.category-filter-bar::-webkit-scrollbar {
    display: none;
}

.category-chip {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    padding: 4px var(--space-3);
    border-radius: 999px;
    border: 1px solid var(--theme-border);
    background: rgba(255, 255, 255, 0.03);
    color: var(--theme-text-dim);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    letter-spacing: 0.04em;
    cursor: pointer;
    white-space: nowrap;
    transition: all var(--duration-fast) var(--ease-out);
}

.category-chip:hover {
    background: rgba(255, 255, 255, 0.06);
    color: var(--theme-text);
}

.category-chip-active {
    background: rgba(122, 162, 247, 0.2);
    border-color: rgba(122, 162, 247, 0.5);
    color: var(--theme-primary, #7aa2f7);
    box-shadow: inset 0 0 0 1px rgba(122, 162, 247, 0.2);
}

.category-chip-label {
    font-size: var(--font-size-xs);
}

.category-chip-count {
    font-size: 10px;
    font-family: var(--font-mono, monospace);
    opacity: 0.7;
}
</style>