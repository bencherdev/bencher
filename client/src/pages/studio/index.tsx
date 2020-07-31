// If you don't want to use TypeScript you can delete this file!
import React from "react"
import { Router, Link } from "@reach/router"

import SitePage from "../../components/site/pages/sitepage"

import Studio from "../../components/studio/studio"
import NewFlow from "../../components/studio/modeler/newflow"
import Modeler from "../../components/studio/modeler/modeler"

const StudioPage = () => (
  <SitePage link={Link}>
    <Router basepath="/studio">
      <Modeler path="/flow/:id" id="" />
      <NewFlow path="/flow" />
      <Studio path="/" />
    </Router>
  </SitePage>
)

export default StudioPage
