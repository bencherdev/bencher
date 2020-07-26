// If you don't want to use TypeScript you can delete this file!
import React from "react"
import { Router } from "@reach/router"

import Layout from "../../components/layout"
import SEO from "../../components/seo"

import Studio from "../../components/studio/studio"
import Modeler from "../../components/studio/modeler/modeler"

const StudioPage = () => (
  <Layout>
    <SEO title="TableFlow Studio" />
    <Router basepath="/studio">
      <Modeler path="/flow/:id" />
      <Modeler path="/flow" />
      <Studio path="/" />
    </Router>
  </Layout>
)

export default StudioPage
