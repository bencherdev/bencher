import React from "react"
import { Card, Icon, Content, Box } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faQuestion, faCog } from "@fortawesome/free-solid-svg-icons"

import DecisionTable from "./DecisionTable"
import Variable from "../variables/Variable"

const Decision = (props: {
  id: string
  value: any
  handleElement: Function
  handleVariable: Function
  getVariable: Function
}) => {
  return (
    <React.Fragment>
      <Card.Content>
        <Content>
          <Box>
            <DecisionTable
              id={props.id}
              value={props.value}
              disabled={false}
              handleElement={props.handleElement}
              handleVariable={props.handleVariable}
              getVariable={props.getVariable}
            />
          </Box>
        </Content>
      </Card.Content>
      <Card.Footer />
      <Card.Content>
        <Content>
          {props?.value?.outputs?.map((variableId: string, index: number) => {
            return (
              <Box key={index}>
                <Variable
                  variable={props.getVariable(variableId)}
                  disabled={{ settings: true, edit: true }}
                  handleVariable={props.handleVariable}
                  getVariable={props.getVariable}
                />
              </Box>
            )
          })}
        </Content>
      </Card.Content>
    </React.Fragment>
  )
}

export default Decision
