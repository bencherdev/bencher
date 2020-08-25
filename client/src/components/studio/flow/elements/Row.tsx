import React from "react"
import { Card, Content, Box, Button, Icon } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faLock, faCog } from "@fortawesome/free-solid-svg-icons"

import Table from "../variables/tabular/Table"

const Row = (props: {
  id: string
  value: any
  handleElement: Function
  handleVariable: Function
  getVariable: Function
}) => {
  const row = props.getVariable(props?.value?.id)
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
            <FontAwesomeIcon icon={faLock} size="2x" />
          </Icon>
        </Card.Header.Icon>
        <Card.Header.Title>{row?.value?.name}</Card.Header.Title>
        <Card.Header.Icon>
          <Icon className="has-text-primary">
            <FontAwesomeIcon icon={faCog} size="2x" />
          </Icon>
        </Card.Header.Icon>
      </Card.Header>
      <Card.Content>
        <Content>
          <Box>
            <Table
              id={row?.id}
              value={row?.value}
              disabled={false}
              handleVariable={props.handleVariable}
            />
            <Button
              color="primary"
              outlined={true}
              size="small"
              fullwidth={true}
              title="Settings"
              onClick={(event: any) => {
                event.preventDefault()
                // props.handleElement()
                console.log("TODO edit settings")
              }}
            >
              <Icon className="primary">
                <FontAwesomeIcon icon={faCog} size="1x" />
              </Icon>
              <span>Settings</span>
            </Button>
          </Box>
        </Content>
      </Card.Content>
    </Card>
  )
}

export default Row
