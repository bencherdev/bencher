import React from "react"
import { navigate } from "gatsby"

import { Card, Columns, Button, Icon, Content } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faPen } from "@fortawesome/free-solid-svg-icons"
import { faCircle } from "@fortawesome/free-regular-svg-icons"

import getFlow from "../../../utils/getFlow"

const Flow = (props: { id: string }) => {
  const flow = getFlow(props.id)
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
                  navigate(`/studio/flow/#${flow?.id?.toLowerCase()}`)
                }}
              >
                <FontAwesomeIcon icon={faCircle} size="2x" />
              </Icon>
            </Card.Header.Icon>
            <Card.Header.Title>{flow?.name}</Card.Header.Title>
            <Card.Header.Icon>
              <Button
                color="primary"
                outlined={true}
                size="small"
                fullwidth={true}
                title="Settings"
                onClick={(event: any) => {
                  event.preventDefault()
                  navigate(`/studio/flow/#${flow?.id?.toLowerCase()}`)
                }}
              >
                <Icon className="primary">
                  <FontAwesomeIcon icon={faPen} size="1x" />
                </Icon>
                <span>Edit</span>
              </Button>
            </Card.Header.Icon>
          </Card.Header>
          <Card.Content>
            <Content>
              <p>{flow?.description}</p>
            </Content>
          </Card.Content>
        </Card>
      </Columns.Column>
    </Columns>
  )
}

export default Flow
