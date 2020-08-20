import React from "react"
import { Card, Content, Button, Icon } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faLock, faCog } from "@fortawesome/free-solid-svg-icons"

import Table from "./variables/table"

const Row = (props: {
  id: string
  value: any
  handleElement: Function
  getElement: Function
}) => {
  const row = props.getElement(props?.value?.id)
  return (
    <Card>
      <Card.Header>
        <Card.Header.Icon className="has-text-primary">
          <FontAwesomeIcon icon={faLock} size="2x" />
        </Card.Header.Icon>
        <Card.Header.Title>{row?.value?.name}</Card.Header.Title>
      </Card.Header>
      <Card.Content>
        <Content>
          <Table
            id={row.id}
            value={row.value}
            disabled={false}
            handleElement={props.handleElement}
          />
          <Button
            color="primary"
            outlined={true}
            size="small"
            fullwidth={true}
            title="Settings"
            onClick={(event: any) => {
              event.preventDefault()
              console.log("TODO edit settings")
            }}
          >
            <Icon className="primary">
              <FontAwesomeIcon icon={faCog} size="1x" />
            </Icon>
            <span>Settings</span>
          </Button>
        </Content>
      </Card.Content>
    </Card>
  )
}

export default Row
