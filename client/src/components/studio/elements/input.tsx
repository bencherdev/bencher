import React from "react"
import { Card, Button, Content } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faArrowRight } from "@fortawesome/free-solid-svg-icons"

const Input = (props: {
  id: number
  value: any
  handleElement: Function
  context: { parent: string; current: string }
  getSubflowName: Function
}) => {
  return (
    <Card>
      <Card.Header>
        <Card.Header.Icon className="has-text-primary">
          <FontAwesomeIcon icon={faArrowRight} size="2x" />
        </Card.Header.Icon>
        <Card.Header.Title>
          Input to {props?.getSubflowName(props?.context?.current)} Subflow
        </Card.Header.Title>
      </Card.Header>
      <Card.Content>
        <Content>
          <p>Input Elements and such</p>
        </Content>
      </Card.Content>
    </Card>
  )
}

export default Input
