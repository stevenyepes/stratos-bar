<template>
  <div class="settings-aliases" data-testid="settings-aliases">
    <div class="d-flex align-center mb-4">
      <div class="section-title">App Aliases</div>
      <v-spacer></v-spacer>
      <v-text-field
        v-model="filterText"
        density="compact"
        variant="outlined"
        hide-details
        placeholder="Filter apps..."
        prepend-inner-icon="mdi-magnify"
        style="max-width: 280px;"
        class="custom-input"
        data-testid="alias-filter"
      ></v-text-field>
    </div>

    <div class="text-caption text-medium-emphasis mb-4" data-testid="alias-help">
      Aliases let you launch apps with custom shortcuts. Use letters, digits, spaces, dots, hyphens, or underscores (1–32 characters).
    </div>

    <div
      v-if="visibleApps.length === 0"
      class="d-flex flex-column align-center justify-center py-12 text-medium-emphasis"
      data-testid="alias-empty"
    >
      <v-icon icon="mdi-alias" size="64" class="mb-4 opacity-50"></v-icon>
      <div class="text-h6 font-weight-regular">No apps match</div>
      <div class="text-caption">Adjust your filter to see more apps.</div>
    </div>

    <div class="aliases-list custom-scrollbar" data-testid="aliases-list">
      <v-card
        v-for="row in visibleApps"
        :key="row.id"
        class="alias-row border-thin mb-2"
        flat
        :data-testid="`alias-row-${row.id}`"
      >
        <div class="d-flex align-center pa-3">
          <v-avatar color="primary" variant="tonal" rounded size="32" class="mr-3">
            <img v-if="row.icon" :src="convertFileSrc(row.icon)" width="22" height="22" />
            <v-icon v-else icon="mdi-application" size="18"></v-icon>
          </v-avatar>
          <div class="flex-grow-1" style="min-width: 0;">
            <div class="text-body-2 font-weight-bold text-truncate">{{ row.name }}</div>
            <div class="text-caption text-medium-emphasis text-truncate font-mono">{{ row.exec }}</div>
          </div>
          <div class="d-flex flex-wrap align-center justify-end" style="max-width: 60%; gap: 6px;">
            <v-chip
              v-for="(alias, idx) in row.aliases"
              :key="alias + idx"
              size="small"
              variant="flat"
              closable
              class="bg-surface-light alias-chip"
              :data-testid="`alias-chip-${row.id}-${alias}`"
              @click:close="handleRemove(row, alias)"
            >{{ alias }}</v-chip>
            <v-text-field
              v-model="row.newAlias"
              density="compact"
              variant="outlined"
              hide-details
              placeholder="add alias"
              style="max-width: 140px;"
              class="custom-input alias-input"
              :data-testid="`alias-input-${row.id}`"
              :error="!!row.aliasError"
              :error-messages="row.aliasError"
              @keydown.enter.prevent="handleAdd(row)"
              @blur="touchRow(row)"
              @update:model-value="validateRow(row)"
            ></v-text-field>
            <v-btn
              size="x-small"
              variant="tonal"
              color="primary"
              class="text-none"
              :disabled="!row.newAlias || !!row.aliasError"
              :data-testid="`alias-add-${row.id}`"
              @click="handleAdd(row)"
            >Add</v-btn>
          </div>
        </div>
        <div
          v-if="row.aliasError"
          class="alias-error text-caption px-3 pb-2"
          :data-testid="`alias-error-${row.id}`"
        >{{ row.aliasError }}</div>
      </v-card>
    </div>
  </div>
</template>

<script setup>
import { computed, reactive, ref, watch } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { useAppIndex, ALIAS_VALIDATION_REGEX } from '../composables/useAppIndex'

const props = defineProps({
    apps: {
        type: Array,
        default: () => [],
    },
})

const emit = defineEmits(['saved', 'error'])

const { aliasesByAppId, setAppAlias } = useAppIndex()

const filterText = ref('')

function validateAliasText(value) {
    if (!value) return ''
    if (!ALIAS_VALIDATION_REGEX.test(value)) {
        return 'Use letters, digits, spaces, dots, hyphens, or underscores (1–32 chars).'
    }
    return ''
}

const rowsById = reactive({})

function buildRows() {
    const list = Array.isArray(props.apps) ? props.apps : []
    const aliasMap = aliasesByAppId.value || {}
    const next = {}
    for (const app of list) {
        const inline = Array.isArray(app.aliases) ? app.aliases : []
        const stored = aliasMap[app.id] || aliasMap[app.exec] || []
        const merged = inline.length ? inline : stored
        const existing = rowsById[app.id]
        if (existing) {
            existing.icon = app.icon
            existing.name = app.name
            existing.exec = app.exec
            const existingAliases = existing.aliases.join('|')
            const mergedAliases = merged.join('|')
            if (existingAliases !== mergedAliases) {
                existing.aliases = [...merged]
            }
            next[app.id] = existing
        } else {
            next[app.id] = reactive({
                id: app.id,
                name: app.name,
                exec: app.exec,
                icon: app.icon,
                aliases: [...merged],
                newAlias: '',
                aliasError: '',
            })
        }
    }
    for (const key of Object.keys(rowsById)) {
        if (!next[key]) delete rowsById[key]
    }
    Object.assign(rowsById, next)
}

watch(() => props.apps, buildRows, { immediate: true })
watch(() => aliasesByAppId.value, buildRows, { deep: true })

const visibleApps = computed(() => {
    const list = Array.isArray(props.apps) ? props.apps : []
    const q = filterText.value.trim().toLowerCase()
    return list
        .map((app) => rowsById[app.id])
        .filter(Boolean)
        .filter((row) => {
            if (!q) return true
            const haystack = `${row.name || ''} ${row.exec || ''} ${(row.aliases || []).join(' ')}`.toLowerCase()
            return haystack.includes(q)
        })
})

function validateRow(row) {
    if (!row) return
    const value = (row.newAlias || '').trim()
    if (!value) {
        row.aliasError = ''
        return
    }
    row.aliasError = validateAliasText(value)
}

function touchRow(row) {
    if (!row) return
    validateRow(row)
}

async function handleAdd(row) {
    const value = (row.newAlias || '').trim()
    const error = validateAliasText(value)
    if (error) {
        row.aliasError = error
        return
    }
    if (row.aliases.includes(value)) {
        row.aliasError = 'Already set'
        return
    }
    try {
        await setAppAlias(row.id, [...row.aliases, value])
        row.aliases = [...row.aliases, value]
        row.newAlias = ''
        row.aliasError = ''
        emit('saved', { appId: row.id, alias: value, action: 'add' })
    } catch (e) {
        row.aliasError = e && e.message ? e.message : String(e)
        emit('error', { appId: row.id, alias: value, error: row.aliasError })
    }
}

async function handleRemove(row, alias) {
    try {
        const next = row.aliases.filter((a) => a !== alias)
        await setAppAlias(row.id, next)
        row.aliases = next
        emit('saved', { appId: row.id, alias, action: 'remove' })
    } catch (e) {
        const msg = e && e.message ? e.message : String(e)
        row.aliasError = msg
        emit('error', { appId: row.id, alias, error: msg })
    }
}
</script>

<style scoped>
.section-title {
    text-transform: uppercase;
    font-size: 0.75rem;
    letter-spacing: 1px;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.5);
}

.aliases-list {
    max-height: 480px;
    overflow-y: auto;
}

.alias-row {
    background: rgba(255, 255, 255, 0.02) !important;
    border: 1px solid rgba(255, 255, 255, 0.06) !important;
}

.alias-chip {
    background-color: rgba(122, 162, 247, 0.12) !important;
    color: var(--theme-primary, #7aa2f7) !important;
}

.alias-input {
    min-width: 120px;
}

.alias-error {
    color: #fca5a5;
}
</style>