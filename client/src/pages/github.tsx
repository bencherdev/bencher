import React from "react"
import { Heading, Section } from "react-bulma-components"

import SEO from "../components/utils/seo"
import ContentPage from "../components/site/pages/contentpage"

const GitHubPage = () => {
  return (
    <React.Fragment>
      <SEO title="GitHub Redirect" />
      {typeof window !== "undefined" &&
        window.location.replace("https://github.com/tableflow/tableflow") && (
          <></>
        )}
      <ContentPage>
        <Heading>Redirecting to GitHub...</Heading>
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

export default GitHubPage
