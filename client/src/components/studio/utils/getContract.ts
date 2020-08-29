import contracts from "../data/contracts"

function getContract(id: string): any {
  return contracts[id]
}

export default getContract
