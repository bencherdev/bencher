import React from "react"
import { Card, Button } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faArrowLeft } from "@fortawesome/free-solid-svg-icons"

const Return = (props: {
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
          <FontAwesomeIcon icon={faArrowLeft} size="2x" />
        </Card.Header.Icon>
        <Card.Header.Title>
          Output from {props?.getSubflowName(props?.context?.current)} Subflow
        </Card.Header.Title>
      </Card.Header>
    </Card>
  )
}

export default Return
