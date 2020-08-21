import React, { useState } from "react"
import {
  Card,
  Heading,
  Icon,
  Button,
  Content,
  Columns,
  Box,
} from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faCircle, faCog, faPlus } from "@fortawesome/free-solid-svg-icons"

import Argument from "./argument"
import Variable from "./variables/variable"

const Subflow = (props: {
  id: string
  value: any
  handleElement: Function
  handleVariable: Function
  getVariable: Function
  getSubflow: Function
}) => {
  const [subflow, setSubflow] = useState(props.getSubflow(props?.value?.id))
  // TODO update the other subflow with changes
  // TODO pass down handleElement for configuring the argument
  return (
    <Card>
      <Card.Header>
        <Card.Header.Icon className="has-text-primary">
          <FontAwesomeIcon icon={faCircle} size="2x" />
        </Card.Header.Icon>
        <Card.Header.Title>{subflow?.name}</Card.Header.Title>
        <Card.Header.Icon>
          <Icon className="has-text-primary">
            <FontAwesomeIcon icon={faCog} size="2x" />
          </Icon>
        </Card.Header.Icon>
      </Card.Header>
      <Card.Content>
        <Columns centered={true}>
          <Columns.Column>
            <Content className="has-text-centered">
              <Heading size={4}>Input</Heading>
            </Content>
            {props?.value?.inputs?.map((variableId: string, index: number) => {
              // TODO Figure out the distinction between
              // variables that are passed into the Input for a Subflow
              // and variables that are set from the Input for a Subflow
              return (
                <Box key={index}>
                  <Argument
                    variable={props.getVariable(variableId)}
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
          <Columns.Column>
            <Content className="has-text-centered">
              <Heading size={4}>Output</Heading>
            </Content>
            {props?.value?.outputs?.map((variableId: string, index: number) => {
              return (
                <Box key={index}>
                  <Variable
                    variable={props.getVariable(variableId)}
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
                // props.handleElement()
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

export default Subflow
