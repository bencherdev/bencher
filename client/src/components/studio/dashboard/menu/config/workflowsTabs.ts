const workflowsTabs = {
  workflows: (path: string) => {
    const workflowsPath = `${path}/workflows`
    return {
      path: workflowsPath,
      active: workflowsPath,
      label: "Workflows",
    }
  },
  new: (path: string) => {
    const workflowsPath = `${path}/workflows`
    return {
      path: `${workflowsPath}/new`,
      active: workflowsPath,
      label: "",
    }
  },
}

export default workflowsTabs
