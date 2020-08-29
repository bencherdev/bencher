// If you don't want to use TypeScript you can delete this file!
import React from "react"
import { Heading } from "react-bulma-components"

import SEO from "../../components/utils/seo"
import ContentPage from "../../components/site/pages/ContentPage"

const ManualPage = () => {
  return (
    <React.Fragment>
      <SEO title="TableFlow Manual" />
      <ContentPage>
        <Heading>TableFlow Manual</Heading>
      </ContentPage>
    </React.Fragment>
  )
}

export default ManualPage
