import React from "react"
import { navigate } from "gatsby"
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

import Argument from "./Argument"
import Variable from "../variables/Variable"

import getFlow from "../../utils/getFlow"
import getFlowSignature from "../../utils/getFlowSignature"

const Function = (props: {
  id: string
  value: any
  handleElement: Function
  handleVariable: Function
  getVariable: Function
}) => {
  const flow = getFlow(props?.value?.id)
  const flowSignature = getFlowSignature(flow)

  function getParameter(io: string, index: number) {
    if (!flow || !flowSignature) {
      return
    }

    let mainSubflowId = flowSignature.main
    let flowVariableId = flowSignature[io]?.[index]
    if (!mainSubflowId || !flowVariableId) {
      return
    }

    return flow.subflows?.[mainSubflowId]?.variables?.[flowVariableId]
  }

  function handleArgument(id: string, value: any) {
    console.log("TODO handle argument")
  }

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
            <FontAwesomeIcon icon={faEquals} size="2x" />
          </Icon>
        </Card.Header.Icon>
        <Card.Header.Title>{flow?.name}</Card.Header.Title>
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
              let variable
              if (variableId && variableId !== "") {
                variable = props?.getVariable(variableId)
              }
              return (
                <Box key={index}>
                  <Argument
                    parameter={
                      !variable ? getParameter("inputs", index) : undefined
                    }
                    handleArgument={handleArgument}
                    variable={variable}
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
