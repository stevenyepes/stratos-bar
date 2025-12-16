import { createApp } from 'vue'
import App from './App.vue'
import vuetify from './plugins/vuetify'

console.log('main.js loaded')

try {
    const app = createApp(App)
    console.log('App created')
    app.use(vuetify)
    console.log('Vuetify plugin added')
    app.mount('#app')
    console.log('App mounted successfully')
} catch (error) {
    console.error('Failed to mount app:', error)
    document.body.innerHTML = `<div style="color: red; padding: 20px;">ERROR: ${error.message}</div>`
}
