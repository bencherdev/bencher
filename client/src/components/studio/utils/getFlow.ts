import flows from "../data/flows"

function getFlow(id: string): any {
  return flows[id]
}

export default getFlow
