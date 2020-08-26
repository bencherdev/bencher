const templatesTabs = {
  dashboard: (path: string, key: string) => {
    return {
      path: `${path}/${key.toLocaleLowerCase()}`,
      label: key,
    }
  },
}

export default templatesTabs
