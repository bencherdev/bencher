import config from "../data/config"

function getConfig(id: string): any {
  return config[id]
}

export default getConfig
