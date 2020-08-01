// If you don't want to use TypeScript you can delete this file!
import React from "react"
import { Link } from "gatsby"

import { Section, Container, Columns, Heading } from "react-bulma-components"

import SitePage from "../components/site/pages/sitepage"

const NotFoundPage = () => {
  let date = Date()
  return (
    <SitePage link={Link}>
      <Section>
        <Container>
          <Columns centered={true}>
            <Columns.Column size="half">
              <Heading>Page Not Found</Heading>
              <p>Rendered at: {date}</p>
            </Columns.Column>
          </Columns>
        </Container>
      </Section>
    </SitePage>
  )
}

export default NotFoundPage
