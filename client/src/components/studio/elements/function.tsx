import React from "react"
import { Card, Button, Content } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faArrowRight, faTable } from "@fortawesome/free-solid-svg-icons"

const Function = (props: {
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
          <p>TODO Function Name</p>
        </Card.Header.Title>
      </Card.Header>
      <Card.Content>
        <Content>
          {props?.value?.params?.map((param: any, index: number) => {
            return (
              <Content key={index} className="has-text-centered">
                <h4>{param.name}</h4>
                <span className="icon has-text-primary">
                  <FontAwesomeIcon icon={faTable} size="2x" />
                </span>
              </Content>
            )
          })}
          <p>Input Elements and such</p>
        </Content>
      </Card.Content>
      <Card.Footer>
        <Content>
          <p>TODO Outputs</p>
        </Content>
      </Card.Footer>
    </Card>
  )
}

export default Function
