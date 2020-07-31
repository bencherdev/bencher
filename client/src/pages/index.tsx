import React from "react"
import { Link } from "gatsby"

import "react-bulma-components/dist/react-bulma-components.min.css"
import "../styles/_variables.sass"

import SitePage from "../components/site/pages/sitepage"

const RootIndex = () => (
  <SitePage link={Link}>
    <h1>TableFlow</h1>
    <Link to="/studio/">Studio</Link> <br />
    <Link to="/404">404</Link> <br />
  </SitePage>
)

export default RootIndex
