import { evaluate } from 'mathjs'

export const MathSkill = {
    id: 'builtin-math',
    name: 'Calculator',
    description: 'Calculate math expressions',
    icon: 'mdi-calculator',

    match(query) {
        // 1. Direct Math Check (e.g. "2+2", "5 * 10")
        // Allow spaces, basic operators, parens.
        // Must contain at least one operator or function to be worth calculating,
        // otherwise "2" matches and shows "2".
        if (/^[\d\.\s\(\)\+\-\*\/\%\^]+$/.test(query)) {
            // Basic heuristic: must have an operator/symbol
            if (/[\+\-\*\/\%\^]/.test(query)) {
                try {
                    const result = evaluate(query)
                    if (typeof result === 'number' && !isNaN(result)) {
                        return { score: 1.0, data: { expression: query, result }, preview: `= ${result}` }
                    }
                } catch (e) {
                    // ignore invalid math
                }
            }
        }

        // 2. Natural Language Parsing
        // "sum of 5 and 10", "product of 5 and 20", "sqrt of 144"
        const nlpExpression = this.preprocessNLP(query)
        if (nlpExpression) {
            try {
                const result = evaluate(nlpExpression)
                if (typeof result === 'number' && !isNaN(result)) {
                    return { score: 0.95, data: { expression: nlpExpression, result }, preview: `= ${result}` }
                }
            } catch (e) {
                // ignore
            }
        }

        return null
    },

    preprocessNLP(query) {
        let q = query.toLowerCase().trim()

        // Map common phrases to operators
        // Order matters: longer phrases first
        const replacements = [
            { from: /plus/g, to: '+' },
            { from: /minus/g, to: '-' },
            { from: /times/g, to: '*' },
            { from: /divided by/g, to: '/' },
            { from: /multiplied by/g, to: '*' },
            { from: /sum of/g, to: '' }, // "sum of A and B" -> "A and B" (handled below)
            { from: /product of/g, to: '' },
            { from: /difference of/g, to: '' }, // tricky: difference of A and B -> A - B or |A-B|? usually A-B
            { from: /square root of/g, to: 'sqrt' },
            { from: /power of/g, to: '^' }, // "5 to the power of 2"
            { from: /\band\b/g, to: '+' }, // "5 and 10" (contextual, usually sum if 'sum' was present, but we'll try)
        ]

        // Heuristic: If it starts with a "command" word, we treat 'and' as the separator for that command
        let isCommand = false
        if (/^(sum|add|product|multiply|difference|subtract|divide|quotient)/.test(q)) {
            isCommand = true
            // If command is explicitly about multiplication/division, we need to map 'and' carefully
            if (/^(product|multiply)/.test(q)) {
                q = q.replace(/\band\b/g, '*')
            } else if (/^(divide|quotient)/.test(q)) {
                q = q.replace(/\band\b/g, '/')
            } else if (/^(difference|subtract)/.test(q)) {
                q = q.replace(/\band\b/g, '-')
            } else {
                // Default to + for "sum", "add"
                q = q.replace(/\band\b/g, '+')
            }
        }

        // Apply general replacements
        for (const { from, to } of replacements) {
            q = q.replace(from, to)
        }

        // Remove "is", "calculate", "what is"
        q = q.replace(/^(calculate|what is|compute)\s+/, '')

        // Final sanity check: does it look like math now?
        // It should explicitly have numbers and operators
        if (/[\d]/.test(q) && /[\+\-\*\/\^a-z\(]/.test(q)) {
            return q
        }

        return null
    },

    async execute(data) {
        // Just return the result, the UI will decide what to do (e.g. copy to clipboard)
        return data.result
    }
}
