<template>
  <div class="omnibar-terminal-mode scale-in">
      <div class="terminal-header" data-tauri-drag-region>
          <div class="d-flex align-center">
              <v-icon icon="mdi-console-line" size="small" class="mr-2 text-success script-pulse"></v-icon>
              <span class="text-subtitle-2 font-mono text-success">Running script...</span>
          </div>
          <v-spacer></v-spacer>
          <div v-if="scriptRunning" class="text-caption text-dimmer mr-2">
              <v-progress-circular indeterminate color="success" size="16" width="2"></v-progress-circular>
          </div>
      </div>
      <div ref="terminalOutputRef" class="terminal-output custom-scrollbar font-mono text-caption">
          <div class="mb-2 text-medium-emphasis">> Executing: {{ currentScript?.alias }} {{ currentScript?.args || '' }}</div>
          <pre class="terminal-text">{{ scriptOutput }}</pre>
          <div v-if="scriptError" class="mt-2 font-weight-bold" style="color: #ef4444;">
              > Error: {{ scriptError }}
          </div>
      </div>
      <div class="terminal-footer">
          <span class="text-caption text-dimmer">[Esc] to Close</span>
      </div>
  </div>
</template>

<script setup>
import { useScriptRunner } from '../../composables/useScriptRunner'

// We need to destructure terminalOutputRef so we can bind it to the template
const { scriptOutput, scriptRunning, scriptError, currentScript, terminalOutputRef } = useScriptRunner()
</script>

<style scoped>
/* Terminal Mode Styles from App.vue */
.omnibar-terminal-mode {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: #0f1115; /* Dark terminal background */
    border-radius: var(--radius-xl);
    overflow: hidden;
    position: relative;
    box-shadow: var(--shadow-xl);
    border: 1px solid var(--theme-border);
}

.terminal-header {
    padding: 12px 16px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    display: flex;
    align-items: center;
}

.terminal-output {
    flex-grow: 1;
    padding: 16px;
    overflow-y: auto;
    font-family: 'JetBrains Mono', 'Fira Code', monospace;
    font-size: 13px;
    line-height: 1.5;
    white-space: pre-wrap;
    color: #e2e8f0;
}

.terminal-footer {
    padding: 8px 16px;
    border-top: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(0, 0, 0, 0.2);
    text-align: right;
}

.script-pulse {
    animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
}

@keyframes pulse {
  0%, 100% {opacity: 1;}
  50% {opacity: 0.5;}
}

/* Utilities reused from App.vue */
.mr-2 { margin-right: var(--space-2); }
.mt-2 { margin-top: var(--space-2); }
.mb-2 { margin-bottom: var(--space-2); }
.d-flex { display: flex; }
.align-center { align-items: center; }
</style>
