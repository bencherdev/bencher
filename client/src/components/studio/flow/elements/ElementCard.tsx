import React, { ReactElement } from "react"
import { Card, Icon } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faCog, IconDefinition } from "@fortawesome/free-solid-svg-icons"

const ElementCard = (props: {
  icon: IconDefinition
  name: string
  children: ReactElement
}) => {
  return (
    <Card>
      <Card.Header>
        <Card.Header.Icon className="has-text-primary">
          <Icon
            className="has-text-primary"
            onClick={(event: Event) => {
              event.preventDefault()
              console.log("TODO add ref to url bar and focus")
            }}
          >
            <FontAwesomeIcon icon={props?.icon} size="2x" />
          </Icon>
        </Card.Header.Icon>
        <Card.Header.Title>{props?.name}</Card.Header.Title>
        <Card.Header.Icon>
          <Icon className="has-text-primary">
            <FontAwesomeIcon icon={faCog} size="2x" />
          </Icon>
        </Card.Header.Icon>
      </Card.Header>
      {props?.children}
    </Card>
  )
}

export default ElementCard
