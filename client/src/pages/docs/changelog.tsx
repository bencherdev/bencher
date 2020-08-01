// If you don't want to use TypeScript you can delete this file!
import React from "react"
import { Link } from "gatsby"

import { Heading } from "react-bulma-components"

import SitePage from "../../components/site/pages/sitepage"
import ContentPage from "../../components/site/pages/contentpage"

const ChangeLogPage = () => {
  return (
    <SitePage link={Link}>
      <ContentPage>
        <Heading>TableFlow Change Log</Heading>
      </ContentPage>
    </SitePage>
  )
}

export default ChangeLogPage
