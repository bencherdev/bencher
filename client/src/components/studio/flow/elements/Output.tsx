import React from "react"
import { Card, Icon, Columns, Box, Button } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faPlus } from "@fortawesome/free-solid-svg-icons"

import Variable from "../variables/Variable"

const Output = (props: {
  id: string
  value: any
  handleElement: Function
  handleVariable: Function
  getVariable: Function
  context: { parent: string; current: string }
}) => {
  return (
    <Card.Content>
      <Columns centered={true} breakpoint="mobile">
        <Columns.Column>
          {props?.value?.outputs?.map((variableId: string, index: number) => {
            return (
              <Box key={index}>
                <Variable
                  variable={props?.getVariable(variableId)}
                  disabled={{ settings: false, edit: true }}
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
            onClick={(event: any) => {
              event.preventDefault()
              // props.handleElement
              console.log("TODO add a new output element")
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

export default Output
