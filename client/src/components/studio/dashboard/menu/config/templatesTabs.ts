const templatesTabs = {
  templates: (path: string) => {
    const templatesPath = `${path}/templates`
    return {
      path: templatesPath,
      active: templatesPath,
      label: "Templates",
    }
  },
  new: (path: string) => {
    const templatesPath = `${path}/templates`
    return {
      path: `${templatesPath}/new`,
      active: templatesPath,
      label: "",
    }
  },
}

export default templatesTabs
