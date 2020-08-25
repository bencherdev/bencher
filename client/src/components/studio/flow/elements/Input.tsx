import React from "react"
import { Card, Button, Columns, Icon, Box } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faPlus } from "@fortawesome/free-solid-svg-icons"

import Variable from "../variables/Variable"

const Input = (props: {
  id: string
  value: any
  handleElement: Function
  handleVariable: Function
  getVariable: Function
  context: { parent: string; current: string }
  getSubflow: Function
}) => {
  return (
    <Card.Content>
      <Columns centered={true}>
        <Columns.Column>
          {props.value?.inputs?.map((variableId: string, index: number) => {
            return (
              <Box key={index}>
                <Variable
                  variable={props?.getVariable(variableId)}
                  disabled={{ settings: false, edit: false }}
                  handleVariable={props.handleVariable}
                  getVariable={props.getVariable}
                />
              </Box>
            )
          })}
          <Button
            color="primary"
            outlined={true}
            fullwidth={true}
            title="Add input"
            onClick={(event: any) => {
              event.preventDefault()
              // props.handleElement()
              console.log("TODO add a new inut element")
            }}
          >
            <Icon className="primary">
              <FontAwesomeIcon icon={faPlus} size="1x" />
            </Icon>
            <span>Add</span>
          </Button>
        </Columns.Column>
      </Columns>
    </Card.Content>
  )
}

export default Input
