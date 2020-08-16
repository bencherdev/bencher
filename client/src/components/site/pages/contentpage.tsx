import React from "react"
import PropTypes from "prop-types"
import {
  Section,
  Container,
  Columns,
  Content,
  Heading,
} from "react-bulma-components"

const ContentPage = ({ children }: any) => {
  return (
    <Section>
      <Container>
        <Columns centered={true}>
          <Columns.Column size="three-quarters">
            <Content>{children}</Content>
          </Columns.Column>
        </Columns>
      </Container>
    </Section>
  )
}

ContentPage.propTypes = {
  children: PropTypes.node.isRequired,
}

export default ContentPage
