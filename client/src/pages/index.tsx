import React from "react"
import { Link } from "gatsby"

import "react-bulma-components/dist/react-bulma-components.min.css"
import "../styles/_variables.sass"

import SitePage from "../components/site/pages/sitepage"
import ContentPage from "../components/site/pages/contentpage"

const RootIndex = () => (
  <SitePage link={Link}>
    <ContentPage>
      <h1>TableFlow</h1>
      <Link to="/studio/">Studio</Link> <br />
      <Link to="/404">404</Link> <br />
    </ContentPage>
  </SitePage>
)

export default RootIndex
