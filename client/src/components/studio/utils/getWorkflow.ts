import workflows from "../data/workflows"

function getWorklow(id: string): any {
  return workflows[id]
}

export default getWorklow
