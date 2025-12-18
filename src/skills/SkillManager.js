
/**
 * Skill Interface:
 * - id: string
 * - name: string
 * - description: string
 * - icon: string (mdi icon)
 * - match(query: string): { score: number, data: any } | null
 * - execute(data: any): Promise<any>
 */

export class SkillManager {
    constructor() {
        this.skills = []
    }

    register(skill) {
        this.skills.push(skill)
    }

    /**
     * Finds the best matching skill for a query.
     * Returns { skill, score, data, preview }
     */
    match(query) {
        if (!query) return null

        let bestMatch = null
        let highestScore = 0

        for (const skill of this.skills) {
            const result = skill.match(query)
            if (result && result.score > highestScore) {
                highestScore = result.score
                bestMatch = {
                    skill,
                    score: result.score,
                    data: result.data,
                    preview: result.preview // Optional preview text (e.g. calculated result)
                }
            }
        }

        // define a threshold, e.g. 0.8 for high confidence
        return bestMatch && bestMatch.score > 0.5 ? bestMatch : null
    }
}
