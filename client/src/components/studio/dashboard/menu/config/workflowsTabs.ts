const workflowsTabs = {
  dashboard: (path: string, key: string) => {
    return {
      path: `${path}/${key.toLocaleLowerCase()}`,
      label: key,
    }
  },
}

export default workflowsTabs
