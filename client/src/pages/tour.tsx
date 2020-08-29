// If you don't want to use TypeScript you can delete this file!
import React from "react"
import { Heading } from "react-bulma-components"

import SEO from "../components/utils/seo"
import ContentPage from "../components/site/pages/ContentPage"

const TourPage = () => {
  return (
    <React.Fragment>
      <SEO title="A Tour of TableFlow" />
      <ContentPage>
        <Heading>Redirecting to A Tour of TableFlow...</Heading>
      </ContentPage>
    </React.Fragment>
  )
}

export default TourPage
