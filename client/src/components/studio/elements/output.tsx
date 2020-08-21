import React from "react"
import { Card, Icon, Columns, Box, Button } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faArrowLeft, faCog, faPlus } from "@fortawesome/free-solid-svg-icons"

import Variable from "./variables/variable"

const Output = (props: {
  id: string
  value: any
  handleElement: Function
  handleVariable: Function
  getVariable: Function
  context: { parent: string; current: string }
  getSubflow: Function
}) => {
  return (
    <Card>
      <Card.Header>
        <Card.Header.Icon className="has-text-primary">
          <FontAwesomeIcon icon={faArrowLeft} size="2x" />
        </Card.Header.Icon>
        <Card.Header.Title>
          Output from {props?.getSubflow(props?.context?.current)?.name} Subflow
        </Card.Header.Title>
        <Card.Header.Icon>
          <Icon className="has-text-primary">
            <FontAwesomeIcon icon={faCog} size="2x" />
          </Icon>
        </Card.Header.Icon>
      </Card.Header>
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
    </Card>
  )
}

export default Output
