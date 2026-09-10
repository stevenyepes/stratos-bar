import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

/**
 * `index.html` carries a `<meta http-equiv="Content-Security-Policy">` and
 * `tauri.conf.json` carries a `csp` that Tauri injects as a response header. A
 * page served with both is subject to both: a resource must satisfy every
 * policy present, so the effective policy is their intersection, and the meta
 * tag can veto a scheme the Tauri config grants.
 *
 * That is not hypothetical. The `stratos-icon` protocol was added to the Tauri
 * config's `img-src` and not to the meta tag, so WebKit blocked every icon
 * request before it reached the Rust handler -- no failed request to log, no
 * broken-image placeholder, and a CSP violation visible only in a devtools
 * console the release build does not open.
 */
const root = resolve(__dirname, '../..')

function imgSrcSources(policy: string): string[] {
    const directive = policy
        .split(';')
        .map((part) => part.trim())
        .find((part) => part.startsWith('img-src'))
    if (!directive) throw new Error(`no img-src directive in: ${policy}`)
    return directive.split(/\s+/).slice(1)
}

describe('Content-Security-Policy agreement', () => {
    const html = readFileSync(resolve(root, 'index.html'), 'utf-8')
    const metaPolicy = /http-equiv="Content-Security-Policy"\s*\n?\s*content="([^"]*)"/.exec(html)?.[1]
    const tauriPolicy = JSON.parse(
        readFileSync(resolve(root, 'src-tauri/tauri.conf.json'), 'utf-8'),
    ).app.security.csp as string

    it('finds a policy in both places', () => {
        expect(metaPolicy).toBeTruthy()
        expect(tauriPolicy).toBeTruthy()
    })

    it('grants every img-src scheme the Tauri config grants', () => {
        const meta = imgSrcSources(metaPolicy as string)
        const schemes = imgSrcSources(tauriPolicy).filter(
            (source) => source.endsWith(':') && !source.includes('//'),
        )
        for (const scheme of schemes) {
            expect(meta, `${scheme} is granted by tauri.conf.json but blocked by index.html`).toContain(scheme)
        }
    })

    it('serves icons over stratos-icon rather than a third-party CDN', () => {
        expect(imgSrcSources(metaPolicy as string)).toContain('stratos-icon:')
        expect(metaPolicy).not.toContain('hatscripts.github.io')
    })
})
