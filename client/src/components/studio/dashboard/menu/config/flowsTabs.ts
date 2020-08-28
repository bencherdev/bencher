const flowsTabs = {
  flows: (path: string) => {
    const flowsPath = `${path}/flows`
    return {
      path: flowsPath,
      active: flowsPath,
      label: "Flows",
    }
  },
  new: (path: string) => {
    const flowsPath = `${path}/flows`
    return {
      path: `${flowsPath}/new`,
      active: flowsPath,
      label: "",
    }
  },
}

export default flowsTabs
