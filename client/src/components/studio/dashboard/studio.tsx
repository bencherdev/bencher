import React, { useState, useEffect } from "react"
import { Section, Columns } from "react-bulma-components"

import SEO from "../../utils/seo"
import Menu from "./menu/Menu"
import Panel from "./panels/Panel"
import FooterPadding from "../../utils/FooterPadding"

import getConfig from "../utils/getConfig"
import getUser from "../../utils/getUser"

function getPath() {
  const pathname = window.location.pathname.split("/")
  return pathname[pathname.length - 1]
}

const Studio = () => {
  const user = getUser()
  const [config, setConfig] = useState(getConfig(user?.id))
  const [path, setPath] = useState(getPath())

  function handleConfig(config: any) {
    setConfig(config)
  }

  useEffect(() => {
    const currentPath = getPath()
    if (currentPath != path) {
      setPath(currentPath)
    }
  }, [])

  return (
    <Section>
      <SEO title="TableFlow Studio" />
      <Columns className="is-reverse-mobile">
        <Columns.Column size="one-fifth">
          <Menu path="/studio" />
        </Columns.Column>
        <Columns.Column>
          <Panel config={config} panel={path} />
        </Columns.Column>
      </Columns>
      <FooterPadding margin={1000} />
    </Section>
  )
}

export default Studio
