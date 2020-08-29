import React from "react"
import { Section, Container, Columns, Content } from "react-bulma-components"

const ContentPage = (props: { children: any }) => {
  return (
    <Section>
      <Container>
        <Columns centered={true}>
          <Columns.Column size="three-quarters">
            <Content>{props?.children}</Content>
          </Columns.Column>
        </Columns>
      </Container>
    </Section>
  )
}

export default ContentPage
