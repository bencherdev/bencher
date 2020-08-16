import React from "react"
import { Card, Button } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faDirections } from "@fortawesome/free-solid-svg-icons"

const Return = (props: {
  id: number
  value: any
  handleElement: Function
  context: { parent: string; subflow: string }
}) => {
  return (
    <Card>
      <Card.Header>
        <Card.Header.Icon className="has-text-primary">
          <FontAwesomeIcon icon={faDirections} size="2x" />
        </Card.Header.Icon>
        <Card.Header.Title>{props?.context?.parent}</Card.Header.Title>
      </Card.Header>
    </Card>
  )
}

export default Return
