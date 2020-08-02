// If you don't want to use TypeScript you can delete this file!
import React from "react"
import { Router } from "@reach/router"

import Studio from "../../components/studio/studio"
import NewFlow from "../../components/studio/modeler/newflow"
import Modeler from "../../components/studio/modeler/modeler"

const StudioPage = () => (
  <Router basepath="/studio">
    <Modeler path="/flow/:id" id="" />
    <NewFlow path="/flow" />
    <Studio path="/" />
  </Router>
)

export default StudioPage
