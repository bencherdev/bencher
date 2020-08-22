import flows from "../../../../flows/flows"

function getFlow(id: string): any {
  return flows[id]
}

export default getFlow
