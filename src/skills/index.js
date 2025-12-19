import { SkillManager } from './SkillManager'
import { MathSkill } from './builtin/MathSkill'
import { CurrencySkill } from './builtin/CurrencySkill'
import { MatugenSkill } from './builtin/MatugenSkill'

const skillManager = new SkillManager()

// Register Built-in Skills
skillManager.register(MathSkill)
skillManager.register(CurrencySkill)
skillManager.register(MatugenSkill)

export default skillManager
