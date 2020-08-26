import React from "react"
import { navigate } from "gatsby"

import { Card, Columns, Button, Icon } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faPen, IconDefinition } from "@fortawesome/free-solid-svg-icons"

const PanelCard = (props: {
  icon: IconDefinition
  to: string
  title: string
  children: React.ReactElement
}) => {
  return (
    <Columns>
      <Columns.Column size={12}>
        <Card>
          <Card.Header>
            <Card.Header.Icon className="has-text-primary">
              <Icon
                className="primary"
                onClick={(event: any) => {
                  event.preventDefault()
                  navigate(props?.to)
                }}
              >
                <FontAwesomeIcon icon={props?.icon} size="2x" />
              </Icon>
            </Card.Header.Icon>
            <Card.Header.Title>{props?.title}</Card.Header.Title>
            <Card.Header.Icon>
              <Button
                color="primary"
                outlined={true}
                size="small"
                fullwidth={true}
                title="Settings"
                onClick={(event: any) => {
                  event.preventDefault()
                  navigate(props?.to)
                }}
              >
                <Icon className="primary">
                  <FontAwesomeIcon icon={faPen} size="1x" />
                </Icon>
                <span>Edit</span>
              </Button>
            </Card.Header.Icon>
          </Card.Header>
          <Card.Content>{props?.children}</Card.Content>
        </Card>
      </Columns.Column>
    </Columns>
  )
}

export default PanelCard
