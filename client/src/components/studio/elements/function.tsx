import React from "react"
import {
  Card,
  Heading,
  Icon,
  Content,
  Columns,
  Box,
} from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faEquals, faCog } from "@fortawesome/free-solid-svg-icons"

import Argument from "./argument"
import Variable from "./variables/variable"

const Function = (props: {
  id: string
  value: any
  handleElement: Function
  handleVariable: Function
  getVariable: Function
}) => {
  // TODO pass down handleElement for configuring the argument
  return (
    <Card>
      <Card.Header>
        <Card.Header.Icon className="has-text-primary">
          <FontAwesomeIcon icon={faEquals} size="2x" />
        </Card.Header.Icon>
        <Card.Header.Title>{props?.value?.name}</Card.Header.Title>
        <Card.Header.Icon>
          <Icon className="has-text-primary">
            <FontAwesomeIcon icon={faCog} size="2x" />
          </Icon>
        </Card.Header.Icon>
      </Card.Header>
      <Card.Content>
        <Columns centered={true}>
          <Columns.Column size="half">
            <Content className="has-text-centered">
              <Heading size={4}>Input</Heading>
            </Content>
            {props?.value?.inputs?.map((variableId: string, index: number) => {
              return (
                <Box key={index}>
                  <Argument
                    variable={props?.getVariable(variableId)}
                    disabled={{ settings: false, edit: true }}
                    handleVariable={props.handleVariable}
                    getVariable={props.getVariable}
                  />
                </Box>
              )
            })}
          </Columns.Column>
          <Columns.Column size="half">
            <Content className="has-text-centered">
              <Heading size={4}>Output</Heading>
            </Content>
            {props?.value?.outputs?.map((variableId: string, index: number) => {
              return (
                <Box key={index}>
                  <Variable
                    variable={props?.getVariable(variableId)}
                    disabled={{ settings: true, edit: true }}
                    handleVariable={props.handleVariable}
                    getVariable={props.getVariable}
                  />
                </Box>
              )
            })}
          </Columns.Column>
        </Columns>
      </Card.Content>
    </Card>
  )
}

export default Function
