// If you don't want to use TypeScript you can delete this file!
import React from "react"
import { Router, Link } from "@reach/router"

import SEO from "../../components/site/seo"

import SiteNavbar from "../../components/site/navbar/sitenavbar"
import SiteFooter from "../../components/site/footer/sitefooter"

import Studio from "../../components/studio/studio"
import NewFlow from "../../components/studio/modeler/newflow"
import Modeler from "../../components/studio/modeler/modeler"

const StudioPage = () => (
  <div>
    <SiteNavbar link={Link} user={{ isAuth: false }} />
    <SEO title="TableFlow Studio" />
    <Router basepath="/studio">
      <Modeler path="/flow/:id" id="" />
      <NewFlow path="/flow" />
      <Studio path="/" />
    </Router>
    <SiteFooter />
  </div>
)

export default StudioPage
