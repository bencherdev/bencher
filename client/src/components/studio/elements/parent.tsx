import React from "react"
import { Card, Button } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faCircle } from "@fortawesome/free-solid-svg-icons"

const Parent = (props: {
  id: number
  value: any
  handleElement: Function
  context: { parent: string; current: string }
  getSubflow: Function
}) => {
  return (
    <Card>
      <Card.Header>
        <Card.Header.Icon>
          <span className="icon has-text-primary">
            <FontAwesomeIcon icon={faCircle} size="2x" />
          </span>
        </Card.Header.Icon>
        <Card.Header.Title>
          {props?.getSubflow(props?.context?.parent)?.name}
        </Card.Header.Title>
      </Card.Header>
    </Card>
  )
}

export default Parent
