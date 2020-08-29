import React from "react"
import { Heading } from "react-bulma-components"

import SEO from "../../utils/seo"
import ContentPage from "./ContentPage"
import FooterPadding from "../../utils/FooterPadding"

const RedirectPage = (props: { title: string; to: string }) => {
  return (
    <React.Fragment>
      <SEO title={`${props.title} Redirect`} />
      {typeof window !== "undefined" && window.location.replace(props.to) && (
        <></>
      )}
      <ContentPage>
        <Heading>{`Redirecting to ${props.title}...`}</Heading>
        <FooterPadding margin={1000} />
      </ContentPage>
    </React.Fragment>
  )
}

export default RedirectPage
