const contractsTabs = {
  contracts: (path: string) => {
    const contractsPath = `${path}/contracts`
    return {
      path: contractsPath,
      active: contractsPath,
      label: "Contracts",
    }
  },
  new: (path: string) => {
    const contractsPath = `${path}/contracts`
    return {
      path: `${contractsPath}/new`,
      active: contractsPath,
      label: "",
    }
  },
}

export default contractsTabs
