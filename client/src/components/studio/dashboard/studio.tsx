import React, { useState } from "react"
import { Section, Columns } from "react-bulma-components"

import SEO from "../../utils/seo"
import Menu from "./menu/Menu"
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
    <Section>
      <SEO title="TableFlow Studio" />
      <Columns className="is-reverse-mobile">
        <Columns.Column size="one-fifth">
          <Menu path="/studio" />
        </Columns.Column>
        <Columns.Column>
          {config?.flows?.map((id: any) => {
            return <Flow key={id} id={id} />
          })}
        </Columns.Column>
      </Columns>
    </Section>
  )
}

export default Studio
