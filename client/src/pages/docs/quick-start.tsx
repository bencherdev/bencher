// If you don't want to use TypeScript you can delete this file!
import React from "react"
import { Heading } from "react-bulma-components"

import SEO from "../../components/utils/seo"
import ContentPage from "../../components/site/pages/contentpage"

const QuickStart = () => {
  return (
    <React.Fragment>
      <SEO title="Quick Start" />
      <ContentPage>
        <Heading>TableFlow Quick Start Guide</Heading>
      </ContentPage>
    </React.Fragment>
  )
}

export default QuickStart
