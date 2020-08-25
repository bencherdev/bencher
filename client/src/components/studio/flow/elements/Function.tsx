import React from "react"
import { Card, Heading, Content, Columns, Box } from "react-bulma-components"

import Argument from "./Argument"
import Variable from "../variables/Variable"

import getFlowSignature from "../../utils/getFlowSignature"

const Function = (props: {
  id: string
  value: any
  flow: any
  handleElement: Function
  handleVariable: Function
  getVariable: Function
}) => {
  const flowSignature = getFlowSignature(props?.flow)

  function getParameter(io: string, index: number) {
    if (!props?.flow || !flowSignature) {
      return
    }

    let mainSubflowId = flowSignature.main
    let flowVariableId = flowSignature[io]?.[index]
    if (!mainSubflowId || !flowVariableId) {
      return
    }

    return props.flow.subflows?.[mainSubflowId]?.variables?.[flowVariableId]
  }

  function handleArgument(id: string, value: any) {
    console.log("TODO handle argument")
  }

  return (
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
  )
}

export default Function
