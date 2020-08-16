// If you don't want to use TypeScript you can delete this file!
import React from "react"
import { Heading } from "react-bulma-components"

import SEO from "../../components/utils/seo"
import ContentPage from "../../components/site/pages/contentpage"

const DocsIndex = () => {
  return (
    <React.Fragment>
      <SEO title="Documentation" />
      <ContentPage>
        <Heading>TableFlow Documentation</Heading>
      </ContentPage>
    </React.Fragment>
  )
}

export default DocsIndex
