import React from "react"
import { Heading, Section } from "react-bulma-components"

import SEO from "../../utils/seo"
import ContentPage from "./ContentPage"

const RedirectPage = (props: { title: string; to: string }) => {
  return (
    <React.Fragment>
      <SEO title={`${props.title} Redirect`} />
      {typeof window !== "undefined" && window.location.replace(props.to) && (
        <></>
      )}
      <ContentPage>
        <Heading>{`Redirecting to ${props.title}...`}</Heading>
        <Section>
          <br />
        </Section>
        <Section>
          <br />
        </Section>
        <Section>
          <br />
        </Section>
        <Section>
          <br />
        </Section>
        <Section>
          <br />
        </Section>
        <Section>
          <br />
        </Section>
      </ContentPage>
    </React.Fragment>
  )
}

export default RedirectPage
