<template>
  <div class="currency-result-card glass-panel fade-in">
    <!-- Main Result Row -->
    <div class="result-main">
      <!-- From Flag -->
      <div class="currency-flag-wrapper">
        <img 
          :src="getFlagUrl(data.from)" 
          class="currency-flag"
          @error="handleFlagError"
        />
        <span class="currency-code text-dim">{{ data.from }}</span>
      </div>

      <!-- Result Text -->
      <div class="conversion-display">
        <div class="result-value text-gradient">
          {{ formattedResult }} <span class="result-unit">{{ data.to }}</span>
        </div>
        <div class="result-sub text-dimmer">
          {{ data.amount }} {{ data.from }} • {{ rateText }}
        </div>
      </div>

      <!-- To Flag -->
      <div class="currency-flag-wrapper">
        <img 
          :src="getFlagUrl(data.to)" 
          class="currency-flag"
          @error="handleFlagError"
        />
        <span class="currency-code text-dim">{{ data.to }}</span>
      </div>
    </div>

    <!-- Metadata Footer -->
    <div class="result-footer">
      <div v-if="data.timestamp" class="timestamp text-dimmer">
        Updated {{ timeAgo(data.timestamp) }}
      </div>
      <div class="actions">
        <!-- Swap Button (Clickable, also triggered by Tab) -->
        <button class="action-chip interactive" @click.stop="$emit('swap')">
          <span class="icon">⇄</span> Swap [Tab]
        </button>
        <!-- Copy Button (Clickable, also triggered by Enter) -->
        <button class="action-chip interactive primary" @click.stop="$emit('execute')">
          <span class="icon">↵</span> Copy
        </button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed } from 'vue'

const props = defineProps({
  data: {
    type: Object,
    required: true
  }
})

// Currency Code to Country Code Mapping for circle-flags
const countryMap = {
  'USD': 'us', 'EUR': 'eu', 'GBP': 'gb', 'JPY': 'jp',
  'CNY': 'cn', 'INR': 'in', 'CAD': 'ca', 'AUD': 'au',
  'CHF': 'ch', 'RUB': 'ru', 'KRW': 'kr', 'BRL': 'br',
  'MXN': 'mx', 'SGD': 'sg', 'HKD': 'hk', 'NZD': 'nz',
  'ZAR': 'za', 'TRY': 'tr', 'SEK': 'se', 'NOK': 'no',
  'COP': 'co', 'ARS': 'ar', 'CLP': 'cl', 'PEN': 'pe', 'UYU': 'uy',
  'PHP': 'ph', 'IDR': 'id', 'THB': 'th', 'MYR': 'my', 'VND': 'vn',
  'BTC': 'btc' // circle-flags supports btc
}

function getFlagUrl(currencyCode) {
  const code = countryMap[currencyCode] || 'xx' // xx is often placeholder
  return `https://hatscripts.github.io/circle-flags/flags/${code}.svg`
}

function handleFlagError(e) {
  e.target.src = 'https://hatscripts.github.io/circle-flags/flags/xx.svg'
}

const formattedResult = computed(() => {
  if (props.data.result === null || props.data.result === undefined) return '...'
  return props.data.result.toLocaleString(undefined, { 
    minimumFractionDigits: 2, 
    maximumFractionDigits: 2 
  })
})

const rateText = computed(() => {
  if (!props.data.rate) return 'Loading rate...'
  return `1 ${props.data.from} = ${props.data.rate.toFixed(4)} ${props.data.to}`
})

function timeAgo(timestamp) {
  if (!timestamp) return ''
  const seconds = Math.floor((Date.now() - timestamp) / 1000)
  if (seconds < 60) return 'just now'
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes}m ago`
  const hours = Math.floor(minutes / 60)
  return `${hours}h ago`
}
</script>

<style scoped>
.currency-result-card {
  background: rgba(0, 0, 0, 0.2);
  border: 1px solid var(--theme-border);
  border-radius: var(--radius-xl);
  padding: var(--space-6);
  margin-bottom: var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.result-main {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
}

.currency-flag-wrapper {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
}

.currency-flag {
  width: 48px;
  height: 48px;
  border-radius: 50%;
  box-shadow: var(--shadow-md);
  object-fit: cover;
}

.currency-code {
  font-size: var(--font-size-xs);
  font-weight: var(--font-weight-bold);
  letter-spacing: 0.05em;
}

.conversion-display {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
}

.result-value {
  font-size: 3rem; /* Large text as requested */
  font-weight: 700;
  line-height: 1.1;
  letter-spacing: -0.02em;
}

.result-unit {
  font-size: 0.5em;
  font-weight: 500;
  color: var(--theme-text-dim);
}

.result-sub {
  font-size: var(--font-size-sm);
  margin-top: var(--space-2);
}

.result-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-top: 1px solid rgba(255, 255, 255, 0.05);
  padding-top: var(--space-3);
}

.actions {
  display: flex;
  gap: var(--space-2);
}

.action-chip {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: 4px 12px;
  border-radius: var(--radius-full);
  background: rgba(255, 255, 255, 0.05);
  font-size: var(--font-size-xs);
  color: var(--theme-text-dim);
  transition: all 0.2s;
}

.action-chip:hover {
  background: rgba(255, 255, 255, 0.1);
  color: var(--theme-text);
}

.action-chip.primary {
  background: rgba(var(--v-theme-primary), 0.1);
  color: rgb(var(--v-theme-primary));
  border: 1px solid rgba(var(--v-theme-primary), 0.2);
}

.action-chip.primary:hover {
  background: rgba(var(--v-theme-primary), 0.2);
}

.icon {
  font-size: 1.1em;
}
</style>
