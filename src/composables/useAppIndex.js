import { ref, shallowRef } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export const ALIAS_VALIDATION_REGEX = /^[A-Za-z0-9_. -]{1,32}$/

export const ALIAS_MAX_LEN = 32

const ALIASES_UPDATED_EVENT = 'aliases-updated'
const APP_INDEX_STATUS_UPDATED_EVENT = 'app-index-status-updated'

const appIndexStatus = ref({
    total_apps: 0,
    indexed_at: null,
    truncated: false,
    disabled_sources: [],
    sources_with_counts: [],
})

const aliasesByAppId = ref({})
const disabledSources = ref([])

const knownAppSources = ['desktop', 'flatpak', 'snap', 'appimage', 'nix', 'other']

function isValidAlias(alias) {
    if (typeof alias !== 'string') return false
    if (alias.length === 0 || alias.length > ALIAS_MAX_LEN) return false
    return ALIAS_VALIDATION_REGEX.test(alias)
}

function filterValidAliases(aliases) {
    if (!Array.isArray(aliases)) return []
    return aliases.filter((a) => typeof a === 'string' && isValidAlias(a))
}

function normalizeDisabledSources(sources) {
    if (!Array.isArray(sources)) return []
    const seen = new Set()
    const out = []
    for (const raw of sources) {
        if (typeof raw !== 'string') continue
        const s = raw.toLowerCase()
        if (!knownAppSources.includes(s)) continue
        if (seen.has(s)) continue
        seen.add(s)
        out.push(s)
    }
    return out
}

function applyAliasesPayload(payload) {
    if (!payload || typeof payload !== 'object') return
    if (Array.isArray(payload.entries)) {
        const next = { ...aliasesByAppId.value }
        for (const entry of payload.entries) {
            if (!entry || typeof entry.app_id !== 'string') continue
            next[entry.app_id] = Array.isArray(entry.aliases) ? [...entry.aliases] : []
        }
        aliasesByAppId.value = next
        return
    }
    if (typeof payload.app_id === 'string') {
        const aliases = Array.isArray(payload.aliases) ? [...payload.aliases] : []
        aliasesByAppId.value = {
            ...aliasesByAppId.value,
            [payload.app_id]: aliases,
        }
    }
}

function applyStatusPayload(status) {
    if (!status || typeof status !== 'object') return
    const disabled = Array.isArray(status.disabled_sources)
        ? status.disabled_sources.map((s) => String(s).toLowerCase()).filter((s) => knownAppSources.includes(s))
        : []
    appIndexStatus.value = {
        total_apps: Number.isFinite(status.total_apps) ? status.total_apps : 0,
        indexed_at: typeof status.indexed_at === 'number' ? status.indexed_at : null,
        truncated: Boolean(status.truncated),
        disabled_sources: disabled,
        sources_with_counts: Array.isArray(status.sources_with_counts) ? status.sources_with_counts : [],
    }
    disabledSources.value = disabled
}

export function useAppIndex() {
    async function listAppIndexStatus() {
        try {
            const status = await invoke('get_app_index_status')
            applyStatusPayload(status)
            return appIndexStatus.value
        } catch (e) {
            console.error('Failed to load app index status', e)
            throw e
        }
    }

    async function setDisabledSources(sources) {
        const normalized = normalizeDisabledSources(sources)
        try {
            const status = await invoke('set_disabled_sources', { sources: normalized })
            applyStatusPayload(status)
            return appIndexStatus.value
        } catch (e) {
            console.error('Failed to set disabled sources', e)
            throw e
        }
    }

    async function setAppAlias(appId, aliases) {
        if (typeof appId !== 'string' || !appId) {
            throw new Error('appId is required')
        }
        const list = Array.isArray(aliases) ? aliases : []
        const invalid = list.filter((a) => !isValidAlias(a))
        if (invalid.length > 0) {
            throw new Error(`invalid alias: ${invalid[0]}`)
        }
        try {
            await invoke('set_app_aliases', { appId, aliases: [...list] })
        } catch (e) {
            console.error('Failed to set app alias', e)
            throw e
        }
    }

    async function bulkSetAliases(map) {
        if (!map || typeof map !== 'object') {
            throw new Error('map of appId -> aliases is required')
        }
        const entries = []
        for (const [appId, aliases] of Object.entries(map)) {
            const list = Array.isArray(aliases) ? aliases : []
            const invalid = list.filter((a) => !isValidAlias(a))
            if (invalid.length > 0) {
                throw new Error(`invalid alias for ${appId}: ${invalid[0]}`)
            }
            entries.push({ app_id: appId, aliases: [...list] })
        }
        try {
            await invoke('bulk_set_aliases', { entries })
        } catch (e) {
            console.error('Failed to bulk set aliases', e)
            throw e
        }
    }

    async function subscribeIndexUpdates() {
        const offAliases = await listen(ALIASES_UPDATED_EVENT, (event) => {
            applyAliasesPayload(event.payload)
        })
        const offStatus = await listen(APP_INDEX_STATUS_UPDATED_EVENT, (event) => {
            const payload = event.payload
            if (payload && payload.status) {
                applyStatusPayload(payload.status)
            } else {
                applyStatusPayload(payload)
            }
        })
        return () => {
            offAliases()
            offStatus()
        }
    }

    return {
        appIndexStatus,
        aliasesByAppId,
        disabledSources,
        isValidAlias,
        filterValidAliases,
        normalizeDisabledSources,
        listAppIndexStatus,
        setDisabledSources,
        setAppAlias,
        bulkSetAliases,
        subscribeIndexUpdates,
    }
}

export function _resetAppIndexForTests() {
    appIndexStatus.value = {
        total_apps: 0,
        indexed_at: null,
        truncated: false,
        disabled_sources: [],
        sources_with_counts: [],
    }
    aliasesByAppId.value = {}
    disabledSources.value = []
}

export const _internals = {
    ALIASES_UPDATED_EVENT,
    APP_INDEX_STATUS_UPDATED_EVENT,
    knownAppSources,
    applyAliasesPayload,
    applyStatusPayload,
}
