// If you don't want to use TypeScript you can delete this file!
import React from "react"
import { Section, Container, Columns, Heading } from "react-bulma-components"

const NotFoundPage = () => {
  let date = Date()
  return (
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
  )
}

export default NotFoundPage
