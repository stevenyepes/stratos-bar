import { describe, it, expect } from 'vitest'
import { SkillManager } from '../SkillManager'

// Mock Skill
const MockSkill = {
    id: 'mock-skill',
    match(query: string) {
        if (query === 'test') return { score: 1.0, data: 'test-data' }
        if (query === 'maybe') return { score: 0.4, data: 'low-score' }
        return null
    }
}

describe('SkillManager', () => {
    it('registers skills', () => {
        const manager = new SkillManager()
        manager.register(MockSkill)
        expect(manager.skills).toHaveLength(1)
    })

    it('finds best matching skill', () => {
        const manager = new SkillManager()
        manager.register(MockSkill)

        const match = manager.match('test')
        expect(match).not.toBeNull()
        expect(match?.skill).toBe(MockSkill)
        expect(match?.score).toBe(1.0)
    })

    it('respects threshold', () => {
        const manager = new SkillManager()
        manager.register(MockSkill)

        // Score 0.4 should be filtered out (threshold 0.5)
        const match = manager.match('maybe')
        expect(match).toBeNull()
    })

    it('returns null for no match', () => {
        const manager = new SkillManager()
        manager.register(MockSkill)

        const match = manager.match('nothing')
        expect(match).toBeNull()
    })
})
