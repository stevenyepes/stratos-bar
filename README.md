# STV Palette

A powerful AI-powered command palette for Linux, built with Tauri, Vue, and Vuetify. Features fast file search, app launching, natural language calculations, AI integration, and customizable themes.

## 🚀 Quick Overview

**STV Palette** is a native Linux application that provides instant access to:
- 🤖 **AI Assistant** (Local Ollama & Cloud OpenAI)
- 🔍 **Fast File & App Search**
- 🧮 **Natural Language Calculator** (with extensible skills system)
- 🎨 **Customizable Themes** (Tokyo Night, Nord, Catppuccin, Custom)
- ⌨️ **Custom Shortcuts** & AI Tools
- 📜 **Script Execution**

**Global Shortcut**: `Super+Space` toggles the command palette

---

## 📋 Table of Contents

- [Tech Stack](#-tech-stack)
- [Features](#-features)
- [Architecture](#-architecture)
- [Project Structure](#-project-structure)
- [Configuration](#-configuration)
- [Development](#-development)
- [Key Components](#-key-components)
- [Skills System](#-skills-system)
- [AI Integration](#-ai-integration)
- [Theme System](#-theme-system)

---

## 🛠 Tech Stack

### Frontend
- **Vue 3** (Composition API with `<script setup>`)
- **Vuetify 3** (Material Design components & theming)
- **Vite** (Build tool & dev server)
- **marked** (Markdown rendering with syntax highlighting)
- **highlight.js** (Code syntax highlighting)
- **mathjs** (Math expression evaluation)

### Backend
- **Tauri 2** (Rust-based native app framework)
- **Rust** (System commands, file search, app launching)
- **Plugins**:
  - `tauri-plugin-global-shortcut` - Global keyboard shortcuts
  - `tauri-plugin-clipboard-manager` - Clipboard operations (Wayland-compatible)
  - `tauri-plugin-single-instance` - Ensure single app instance
  - `tauri-plugin-process` - Process management
  - `tauri-plugin-opener` - File/URL opening

### Dependencies
- **walkdir** - Recursive directory traversal
- **reqwest** - HTTP client for AI API calls
- **serde/serde_json** - Serialization
- **tokio** - Async runtime
- **open** - Cross-platform file opener

---

## ✨ Features

### 1. AI Assistant
- **Local Models**: Ollama integration with dynamic model selection
- **Cloud Models**: OpenAI API support
- **AI Tools**: Extensible tools with keyword triggers (e.g., "rephrase" for text improvement)
- **Context-Aware**: Can access selected text via clipboard
- **Markdown Chat**: Formatted responses with code highlighting

### 2. Application Launcher
- Scans `.desktop` files from `/usr/share/applications` and `~/.local/share/applications`
- Icon resolution from `/usr/share/icons`, `/usr/share/pixmaps`
- Fast fuzzy search by app name or executable

### 3. File Search
- Real-time file search with debouncing (300ms)
- Configurable home directory search
- Quick file opening with default applications

### 4. Skills System
- **Extensible Architecture**: Plugin-like skill registration
- **Built-in MathSkill**:
  - Direct math expressions: `5 * 10`, `sqrt(144)`
  - Natural language: `sum of 5 and 10`, `product of 3 and 7`
  - Automatic clipboard copy on execution
- **Easy to Extend**: See [Skills System](#-skills-system)

### 5. Script Execution
- Execute custom scripts from `~/scripts/`
- Direct integration into command palette

### 6. Theme Customization
- **Preset Themes**: Tokyo Night (default), Nord, Catppuccin Mocha
- **Custom Themes**: Full color customization (primary, secondary, background, surface, text)
- **CSS Variables**: Dynamic theme application
- **Persistent**: Theme preferences saved to config

### 7. Settings Management
- Modern, glassmorphic settings UI
- Persistent configuration in `~/.config/stv-palette/config.json`
- Dynamic Ollama model fetching
- Keyboard shortcuts configuration
- Theme selection & opacity control

---

## 🏗 Architecture

### Application Flow

```mermaid
graph TB
    A[User: Super+Space] --> B[Tauri Window Toggle]
    B --> C[App.vue Main Interface]
    C --> D[Search Input]
    D --> E{Query Type}
    E -->|AI Tool/Skill| F[SkillManager.match]
    E -->|App Name| G[App Launcher]
    E -->|File Name| H[File Search]
    E -->|General Text| I[AI Assistant]
    F --> J[Execute Skill]
    G --> K[Launch App]
    H --> L[Open File]
    I --> M[AiChat.vue]
    M --> N[Rust: ask_ai]
    N --> O{Model Type}
    O -->|Local| P[Ollama API]
    O -->|Cloud| Q[OpenAI API]
```

### Frontend Architecture

**Main Components:**
1. **App.vue** - Root component with:
   - Search bar & query handling
   - Dynamic window resizing based on content
   - Tool/App/File/Skill matching logic
   - AI chat integration
   
2. **AiChat.vue** - Side panel for AI conversations:
   - Message history
   - Markdown rendering with code highlighting
   - Auto-scroll
   - Copy-to-clipboard for code blocks

3. **Settings.vue** - Configuration interface:
   - AI model selection (local/cloud)
   - Theme customization
   - Shortcut management
   - App launcher configuration

### Backend Architecture (Rust)

**Key Modules:**

1. **config.rs** - Configuration management
   - `AppConfig` struct (API keys, models, tools, shortcuts, theme)
   - `ThemeConfig` struct (color schemes)
   - `ConfigManager` (load/save to `~/.config/stv-palette/config.json`)

2. **lib.rs** - Tauri commands (invokable from frontend):
   - `list_apps()` - Parse `.desktop` files
   - `launch_app(exec_cmd)` - Execute applications
   - `search_files(query, path)` - File search with WalkDir
   - `list_scripts()` - List scripts from `~/scripts/`
   - `ask_ai(messages)` - Route to Ollama or OpenAI
   - `list_ollama_models()` - Fetch available Ollama models
   - `get_selection_context()` - Get clipboard text (Wayland-compatible)
   - `copy_to_clipboard(text)` - Copy to clipboard (Wayland-compatible)
   - `get_config()` / `save_config(config)` - Config persistence

3. **main.rs** - Entry point with:
   - Global shortcut registration (`Super+Space`)
   - Single instance enforcement
   - Window management

---

## 📁 Project Structure

```
stv-palette/
├── src/                          # Frontend (Vue)
│   ├── App.vue                   # Main application UI
│   ├── main.js                   # Vue app initialization
│   ├── theme.js                  # CSS theme variable injection
│   ├── components/
│   │   ├── AiChat.vue            # AI chat interface
│   │   └── Settings.vue          # Settings dialog
│   ├── skills/                   # Extensible skills system
│   │   ├── index.js              # Skill manager export
│   │   ├── SkillManager.js       # Skill matching/execution
│   │   └── builtin/
│   │       └── MathSkill.js      # Natural language calculator
│   └── plugins/
│       └── vuetify.js            # Vuetify configuration
│
├── src-tauri/                    # Backend (Rust)
│   ├── src/
│   │   ├── main.rs               # App entry point
│   │   ├── lib.rs                # Tauri commands
│   │   └── config.rs             # Configuration management
│   ├── Cargo.toml                # Rust dependencies
│   └── tauri.conf.json           # Tauri configuration
│
├── package.json                  # Frontend dependencies
├── vite.config.js                # Vite configuration
└── README.md                     # This file
```

---

## ⚙ Configuration

### Config File Location
`~/.config/stv-palette/config.json`

### Config Structure

```json
{
  "openai_api_key": "sk-...",
  "local_model_url": "http://localhost:11434",
  "preferred_model": "local",
  "ollama_model": "llama3",
  "ai_tools": [
    {
      "id": "rephrase",
      "name": "Rephrase Selection",
      "description": "Improve clarity and grammar",
      "prompt_template": "...",
      "keywords": ["rephrase", "rewrite", "fix"],
      "icon": "mdi-pencil-outline"
    }
  ],
  "shortcuts": {
    "calc": "builtin-math",
    "chat": "app:firefox"
  },
  "theme": {
    "name": "Tokyo Night",
    "primary": "#7aa2f7",
    "secondary": "#bb9af7",
    "background": "#1a1b26",
    "surface": "#24283b",
    "text": "#c0caf5",
    "is_custom": false
  }
}
```

### AI Models

**Local (Ollama)**:
- Requires Ollama running on `localhost:11434`
- Models auto-detected from `/api/tags` endpoint
- Default: `llama3`

**Cloud (OpenAI)**:
- Requires `openai_api_key` in config
- Uses `gpt-4` model
- API calls to `https://api.openai.com/v1/chat/completions`

---

## 💻 Development

### Prerequisites
- Node.js (v18+)
- Rust (latest stable)
- Tauri CLI
- Ollama (optional, for local AI)

### Setup

```bash
# Clone the repository
git clone <repo-url>
cd stv-palette

# Install frontend dependencies
npm install

# Run development server
npm run dev

# In another terminal, run Tauri dev
npm run tauri dev
```

### Building

```bash
# Build for production
npm run build
npm run tauri build
```

### Development Tips

1. **Hot Reload**: Frontend changes auto-reload with Vite
2. **Rust Changes**: Require Tauri dev restart
3. **Console Logs**: Check DevTools (Ctrl+Shift+I in dev mode)
4. **Wayland Issues**: Set `WEBKIT_DISABLE_COMPOSITING_MODE=1` if needed

---

## 🔑 Key Components

### App.vue

**Responsibilities:**
- Main search interface
- Query matching logic (tools, apps, files, skills)
- Window resizing based on content & AI chat state
- Keyboard navigation (Up/Down/Enter/Esc)
- Integration with Settings & AiChat components

**Key Computed Properties:**
- `matchedTool` - Determines if query matches a skill, AI tool, or app shortcut
- `filteredApps` - Apps matching search query
- `filteredScripts` - Scripts matching search query

**Key Methods:**
- `executeAction(index)` - Route to appropriate handler based on selection
- `updateWindowSize(expanded)` - Dynamic window resizing with monitor scaling
- `askAI()` - Open AI chat with current query

### AiChat.vue

**Features:**
- Markdown rendering with `marked` + `highlight.js`
- Code block copy-to-clipboard
- Auto-scrolling to latest message
- Loading states
- Message history

**Implementation Notes:**
- Custom `marked` renderer for styled code blocks
- Click handler for copy buttons in rendered HTML
- `nextTick` for proper scroll timing

### Settings.vue

**Sections:**
1. **General**: Model selection (local/cloud), Ollama model picker
2. **Appearance**: Theme selection (presets + custom colors), opacity
3. **AI Tools**: Custom tools with keywords & prompts
4. **Shortcuts**: Key trigger → tool/app mapping

**Implementation Notes:**
- Dynamic Ollama model fetching on mount
- Color pickers for custom theme creation
- Real-time config persistence via `save_config` command

---

## 🧩 Skills System

### Architecture

Skills are self-contained modules with:
- **Interface**: `id`, `name`, `description`, `icon`, `match(query)`, `execute(data)`
- **Registration**: Via `SkillManager.register(skill)`
- **Matching**: Score-based (0-1), threshold of 0.5 required
- **Execution**: Async, returns result (copied to clipboard automatically)

### Creating a New Skill

```javascript
// src/skills/builtin/MySkill.js
export const MySkill = {
    id: 'builtin-my-skill',
    name: 'My Skill',
    description: 'Does something cool',
    icon: 'mdi-star',

    match(query) {
        // Return { score: 0-1, data: any, preview: string } or null
        if (query.startsWith('cool')) {
            return { 
                score: 0.95, 
                data: { input: query }, 
                preview: 'Processing...' 
            }
        }
        return null
    },

    async execute(data) {
        // Return result to copy to clipboard
        return `Processed: ${data.input}`
    }
}

// src/skills/index.js
import { MySkill } from './builtin/MySkill'
skillManager.register(MySkill)
```

### Built-in MathSkill

**Capabilities:**
- Direct expressions: `2+2`, `sqrt(144)`, `5^3`
- Natural language: `sum of 5 and 10`, `product of 3 and 7`
- Complex: `(5 + 3) * 2`, `sqrt of 144`

**Implementation:**
- Regex-based expression detection
- NLP preprocessing for keyword replacement
- `mathjs` evaluation
- Score: 1.0 for direct math, 0.95 for NLP

---

## 🤖 AI Integration

### Flow

1. User types query
2. `matchedTool` checks for keyword/shortcut match
3. If tool found, substitute `{{selection}}` with clipboard content
4. Send prompt to `ask_ai` Rust command
5. Route to Ollama or OpenAI based on `preferred_model`
6. Display response in AiChat.vue with markdown rendering

### Ollama Integration

**Endpoint**: `POST http://localhost:11434/api/chat`

**Request:**
```json
{
  "model": "llama3",
  "messages": [
    {"role": "user", "content": "Hello"}
  ],
  "stream": false
}
```

### OpenAI Integration

**Endpoint**: `POST https://api.openai.com/v1/chat/completions`

**Request:**
```json
{
  "model": "gpt-4",
  "messages": [
    {"role": "user", "content": "Hello"}
  ]
}
```

### Selection Context

**Wayland-Compatible Clipboard Access:**
- Uses `wl-paste` for Wayland
- Falls back to `xclip` for X11
- Triggered by AI tools with `{{selection}}` placeholder

---

## 🎨 Theme System

### Architecture

1. **Backend**: `ThemeConfig` struct in `config.rs`
2. **Frontend**: 
   - `theme.js` - Injects CSS variables
   - Vuetify theme API - Applies colors to components
3. **Persistence**: Saved in `config.json`

### Theme Application Flow

```mermaid
graph LR
    A[Theme Selected] --> B[save_config Rust command]
    B --> C[config.json updated]
    C --> D[config.value updated]
    D --> E[applyTheme CSS vars]
    D --> F[updateVuetifyTheme colors]
```

### CSS Variables

Set by `theme.js`:
- `--theme-background`
- `--theme-surface`
- `--theme-primary`
- `--theme-secondary`
- `--theme-text`

Used in `App.vue` and `AiChat.vue` styles.

### Preset Themes

1. **Tokyo Night** (default)
   - Primary: `#7aa2f7` (blue)
   - Secondary: `#bb9af7` (purple)
   - Background: `#1a1b26` (dark blue-gray)

2. **Nord**
   - Primary: `#88c0d0` (frost blue)
   - Secondary: `#81a1c1` (polar blue)
   - Background: `#2e3440` (dark gray)

3. **Catppuccin Mocha**
   - Primary: `#89b4fa` (blue)
   - Secondary: `#cba6f7` (mauve)
   - Background: `#1e1e2e` (dark)

---

## 🔧 Troubleshooting

### Wayland Issues
- **Clipboard**: Ensure `wl-clipboard` is installed
- **Transparency**: Application uses opaque background (transparent mode caused ghosting)

### Ollama Not Found
- Verify Ollama is running: `curl http://localhost:11434/api/tags`
- Check `local_model_url` in config

### Window Not Showing
- Check global shortcut conflicts (Super+Space)
- Verify single instance isn't blocking

### Settings Not Persisting
- Check file permissions: `~/.config/stv-palette/config.json`
- Verify `save_config` is called after changes

---

## 📝 Future Development Ideas

- [ ] Add more built-in skills (unit conversion, date/time, base conversion)
- [ ] File content preview in search results
- [ ] Browser bookmark integration
- [ ] SSH connection manager
- [ ] Clipboard history
- [ ] Window switcher (wmctrl integration)
- [ ] Plugin marketplace for community skills
- [ ] Streaming AI responses
- [ ] Multi-language support

---

## 🤝 Contributing

This is a personal project, but suggestions and issues are welcome!

---

## 📄 License

[Add your license here]

---

## 🙏 Acknowledgments

Built with:
- [Tauri](https://tauri.app/) - Rust-based desktop framework
- [Vue](https://vuejs.org/) - Progressive JavaScript framework
- [Vuetify](https://vuetifyjs.com/) - Material Design component library
- [Ollama](https://ollama.ai/) - Local LLM runtime
