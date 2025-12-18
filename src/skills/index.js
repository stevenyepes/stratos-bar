import { SkillManager } from './SkillManager'
import { MathSkill } from './builtin/MathSkill'
import { CurrencySkill } from './builtin/CurrencySkill'

const skillManager = new SkillManager()

// Register Built-in Skills
skillManager.register(MathSkill)
skillManager.register(CurrencySkill)

export default skillManager
