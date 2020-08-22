// If you don't want to use TypeScript you can delete this file!
import React from "react"
import { Heading } from "react-bulma-components"

import SEO from "../../components/utils/seo"
import ContentPage from "../../components/site/pages/ContentPage"

const FAQPage = () => {
  return (
    <React.Fragment>
      <SEO title="FAQ" />
      <ContentPage>
        <Heading>TableFlow FAQ</Heading>
      </ContentPage>
    </React.Fragment>
  )
}

export default FAQPage
