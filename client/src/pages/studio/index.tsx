// If you don't want to use TypeScript you can delete this file!
import React from "react"
import { Router } from "@reach/router"

import Studio from "../../components/studio/workspace/studio"
import NewFlow from "../../components/studio/modeler/newflow"
import Modeler from "../../components/studio/modeler/modeler"

const StudioPage = () => (
  <Router basepath="/studio">
    <NewFlow path="/flow/new" />
    <Modeler path="/flow" />
    <Studio path="/" />
  </Router>
)

export default StudioPage
