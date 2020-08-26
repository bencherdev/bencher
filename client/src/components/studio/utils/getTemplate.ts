import templates from "../data/templates"

function getTemplate(id: string): any {
  return templates[id]
}

export default getTemplate
