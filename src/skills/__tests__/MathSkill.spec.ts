import { describe, it, expect } from 'vitest'
import { MathSkill } from '../builtin/MathSkill'

describe('MathSkill', () => {
    it('matches direct math expressions', () => {
        expect(MathSkill.match('2+2')).not.toBeNull()
        expect(MathSkill.match('5 * 10')).not.toBeNull()
        expect(MathSkill.match('sqrt(144)')).not.toBeNull()

        const match = MathSkill.match('2 + 2')
        expect(match?.score).toBe(1.0)
        expect(match?.data.result).toBe(4)
    })

    it('matches natural language expressions', () => {
        const queries = [
            { q: 'sum of 5 and 10', expected: 15 },
            { q: 'product of 3 and 7', expected: 21 },
            { q: 'divide 100 by 4', expected: 25 },
        ]

        for (const { q, expected } of queries) {
            const match = MathSkill.match(q)
            expect(match).not.toBeNull()
            expect(match?.score).toBe(0.95)
            expect(match?.data.result).toBe(expected)
        }
    })

    it('returns null for invalid math', () => {
        expect(MathSkill.match('hello world')).toBeNull()
        expect(MathSkill.match('open firefox')).toBeNull()
    })

    it('executes and returns result', async () => {
        const result = await MathSkill.execute({ result: 42 })
        expect(result).toBe(42)
    })
})
