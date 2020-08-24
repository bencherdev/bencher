const templatesTabs = {
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

export default templatesTabs
