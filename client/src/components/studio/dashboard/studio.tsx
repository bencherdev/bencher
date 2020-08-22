import React, { useState } from "react"
import { Link } from "@reach/router"

import SEO from "../../utils/seo"
import Flow from "./Flow"

import getConfig from "../utils/getConfig"
import getUser from "../../utils/getUser"

const Studio = () => {
  const user = getUser()
  const [config, setConfig] = useState(getConfig(user?.id))

  function handleConfig(config: any) {
    setConfig(config)
  }

  return (
    <div>
      <SEO title="TableFlow Studio" />
      <h1>TableFlow Studio</h1>
      {config?.flows?.map((id: any) => {
        return <Flow key={id} id={id} />
      })}
      <Link to="/studio/flow/new">New Flow</Link>
    </div>
  )
}

export default Studio
