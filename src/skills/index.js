import { SkillManager } from './SkillManager'
import { MathSkill } from './builtin/MathSkill'

const skillManager = new SkillManager()

// Register Built-in Skills
skillManager.register(MathSkill)

export default skillManager
