// If you don't want to use TypeScript you can delete this file!
import React from "react"
import { Heading } from "react-bulma-components"
import SEO from "../components/utils/seo"

import ContentPage from "../components/site/pages/contentpage"

const TermsPage = () => {
  return (
    <React.Fragment>
      <SEO title="Terms of Use" />
      <ContentPage>
        <Heading>TableFlow Terms of Use</Heading>
      </ContentPage>
    </React.Fragment>
  )
}

export default TermsPage
