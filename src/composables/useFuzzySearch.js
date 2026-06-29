const WEIGHT_NAME_PREFIX = 0.20
const WEIGHT_NAME_CONTAINS = 0.15
const WEIGHT_NAME_SUBSEQUENCE = 0.10
const WEIGHT_ALIAS_EXACT = 0.20
const WEIGHT_ALIAS_PREFIX = 0.10
const WEIGHT_ALIAS_CONTAINS = 0.07
const WEIGHT_ALIAS_SUBSEQUENCE = 0.05
const WEIGHT_FREQUENCY_DECAY = 0.05

const MS_PER_DAY = 86_400_000
const DECAY_HALFLIFE_DAYS = 14

const DEFAULT_LIMIT = 25

function normalizeString(value) {
    if (typeof value !== 'string') return ''
    return value.trim().toLowerCase()
}

function isSubsequence(needle, haystack) {
    if (!needle) return true
    if (!haystack) return false
    let i = 0
    for (let j = 0; j < haystack.length && i < needle.length; j += 1) {
        if (haystack[j] === needle[i]) i += 1
    }
    return i === needle.length
}

function subsequenceRatio(needle, haystack) {
    if (!needle || !haystack) return 0
    if (needle === haystack) return 1
    if (!isSubsequence(needle, haystack)) return 0
    return Math.min(1, needle.length / Math.max(haystack.length, 1))
}

function frequencyDecay(launchCount, lastLaunchedAt, now) {
    if (!launchCount || launchCount <= 0) return 0
    if (!lastLaunchedAt) return 0
    const reference = typeof now === 'number' ? now : Date.now()
    const ageDays = Math.max(0, (reference - lastLaunchedAt) / MS_PER_DAY)
    const decay = Math.exp(-ageDays / DECAY_HALFLIFE_DAYS)
    return Math.min(1, decay) * Math.min(1, Math.log1p(launchCount) / 10)
}

function getAliasesForEntry(entry, aliasCtx) {
    if (!entry) return []
    const inline = Array.isArray(entry.aliases) ? entry.aliases : []
    if (aliasCtx && entry && entry.id && Array.isArray(aliasCtx[entry.id])) {
        const merged = [...inline]
        for (const a of aliasCtx[entry.id]) {
            if (!merged.includes(a)) merged.push(a)
        }
        return merged
    }
    return [...inline]
}

export function scoreEntry(entry, query, ctx) {
    if (!entry || typeof entry !== 'object') return 0
    const aliasCtx = ctx && ctx.aliases ? ctx.aliases : null
    const now = ctx && typeof ctx.now === 'number' ? ctx.now : Date.now()
    const launchCount = ctx && typeof ctx.launchCount === 'number'
        ? ctx.launchCount
        : (typeof entry.launch_count === 'number' ? entry.launch_count : 0)
    const lastLaunchedAt = ctx && typeof ctx.lastLaunchedAt === 'number'
        ? ctx.lastLaunchedAt
        : (typeof entry.last_launched_at === 'number' ? entry.last_launched_at : 0)

    const q = normalizeString(query)
    const name = normalizeString(entry.name)
    const aliases = getAliasesForEntry(entry, aliasCtx).map(normalizeString).filter(Boolean)

    if (!q) {
        return WEIGHT_FREQUENCY_DECAY * frequencyDecay(launchCount, lastLaunchedAt, now)
    }

    let score = 0
    if (name) {
        if (name === q) {
            score += WEIGHT_NAME_PREFIX + WEIGHT_NAME_CONTAINS
        } else if (name.startsWith(q)) {
            score += WEIGHT_NAME_PREFIX
        } else if (name.includes(q)) {
            score += WEIGHT_NAME_CONTAINS
        } else if (isSubsequence(q, name)) {
            score += WEIGHT_NAME_SUBSEQUENCE * subsequenceRatio(q, name)
        }
    }

    for (const alias of aliases) {
        if (alias === q) {
            score += WEIGHT_ALIAS_EXACT
        } else if (alias.startsWith(q)) {
            score += WEIGHT_ALIAS_PREFIX
        } else if (alias.includes(q)) {
            score += WEIGHT_ALIAS_CONTAINS
        } else if (isSubsequence(q, alias)) {
            score += WEIGHT_ALIAS_SUBSEQUENCE * subsequenceRatio(q, alias)
        }
    }

    score += WEIGHT_FREQUENCY_DECAY * frequencyDecay(launchCount, lastLaunchedAt, now)
    return score
}

function buildContext(entry, ctx) {
    if (!ctx) return null
    const launchCount = typeof ctx.launchCount === 'number'
        ? ctx.launchCount
        : (typeof entry.launch_count === 'number' ? entry.launch_count : 0)
    const lastLaunchedAt = typeof ctx.lastLaunchedAt === 'number'
        ? ctx.lastLaunchedAt
        : (typeof entry.last_launched_at === 'number' ? entry.last_launched_at : 0)
    return {
        aliases: ctx.aliases,
        launchCount,
        lastLaunchedAt,
        now: ctx.now,
    }
}

export function rankEntries(entries, query, limit, ctx) {
    if (!Array.isArray(entries) || entries.length === 0) return []
    const effectiveLimit = typeof limit === 'number' && limit > 0 ? limit : DEFAULT_LIMIT
    const q = normalizeString(query)
    const scored = []
    for (const entry of entries) {
        const entryCtx = buildContext(entry, ctx)
        const score = scoreEntry(entry, q, entryCtx)
        if (q && score <= 0) continue
        scored.push({ entry, score })
    }
    scored.sort((a, b) => {
        if (b.score !== a.score) return b.score - a.score
        const an = normalizeString(a.entry && a.entry.name)
        const bn = normalizeString(b.entry && b.entry.name)
        if (an && bn && an !== bn) return an.localeCompare(bn)
        return 0
    })
    if (scored.length <= effectiveLimit) return scored
    return scored.slice(0, effectiveLimit)
}

export function filterByCategory(entries, category) {
    if (!Array.isArray(entries)) return []
    const target = normalizeString(category)
    if (!target) return entries.slice()
    return entries.filter((entry) => {
        if (!entry) return false
        const cats = Array.isArray(entry.categories) ? entry.categories : []
        for (const c of cats) {
            if (normalizeString(c) === target) return true
        }
        if (typeof entry.category === 'string' && normalizeString(entry.category) === target) {
            return true
        }
        return false
    })
}

export const _internals = {
    WEIGHT_NAME_PREFIX,
    WEIGHT_NAME_CONTAINS,
    WEIGHT_NAME_SUBSEQUENCE,
    WEIGHT_ALIAS_EXACT,
    WEIGHT_ALIAS_PREFIX,
    WEIGHT_ALIAS_CONTAINS,
    WEIGHT_ALIAS_SUBSEQUENCE,
    WEIGHT_FREQUENCY_DECAY,
    DECAY_HALFLIFE_DAYS,
    isSubsequence,
    subsequenceRatio,
    frequencyDecay,
}
