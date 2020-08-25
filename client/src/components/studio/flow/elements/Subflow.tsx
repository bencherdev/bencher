import React from "react"
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
import { faPlus } from "@fortawesome/free-solid-svg-icons"

import Argument from "./Argument"
import Variable from "../variables/Variable"

import getSubflowSignature from "../../utils/getSubflowSignature"

const Subflow = (props: {
  id: string
  value: any
  subflow: any
  handleElement: Function
  handleVariable: Function
  getVariable: Function
  getSubflow: Function
}) => {
  // TODO update the other subflow with changes
  // TODO pass down handleElement for configuring the argument
  const subflowSignature = getSubflowSignature(props?.subflow)

  function getParameter(io: string, index: number) {
    if (!props?.subflow || !subflowSignature) {
      return
    }

    let subflowVariableId = subflowSignature[io]?.[index]
    if (!subflowVariableId) {
      return
    }

    return props.subflow?.variables?.[subflowVariableId]
  }

  function handleArgument(id: string, value: any) {
    console.log("TODO handle argument")
  }

  return (
    <Card.Content>
      <Columns centered={true}>
        <Columns.Column>
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
  )
}

export default Subflow
