const flowsTabs = {
  dashboard: (path: string, key: string) => {
    return {
      path: `${path}/${key.toLocaleLowerCase()}`,
      label: key,
    }
  },
  new: (path: string, key: string) => {
    return {
      path: `${path}/${key.toLocaleLowerCase()}/new`,
      label: null,
    }
  },
}

export default flowsTabs
