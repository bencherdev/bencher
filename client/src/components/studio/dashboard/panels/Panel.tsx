import React from "react"
import { Router } from "@reach/router"

import Dashboard from "./Dashboard"
import New from "./New"
import Workflows from "./Workflows"
import Flows from "./Flows"
import Templates from "./Templates"

const Panel = (props: { config: any }) => {
  return (
    <Router basepath="/studio">
      <New path="/workflows/new" />
      <Workflows path="/workflows" workflows={props?.config?.workflows} />
      <New path="/flows/new" />
      <Flows path="/flows" flows={props?.config?.flows} />
      <New path="/templates/new" />
      <Templates path="templates" templates={props?.config?.templates} />
      <New path="/contracts/new" />
      {/* TODO Contracts */}
      <Dashboard path="/" />
    </Router>
  )
}

export default Panel
