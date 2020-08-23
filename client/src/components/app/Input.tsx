import React from "react"
import { Section, Columns, Heading } from "react-bulma-components"

const Input = (props: { flow: any }) => {
  return (
    <Section>
      <Columns centered={true}>
        <Columns.Column size="three-quarters">
          <Heading size={2}>Flow Input</Heading>
        </Columns.Column>
      </Columns>
    </Section>
  )
}

export default Input
