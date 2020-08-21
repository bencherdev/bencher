import React from "react"
import { Card, Content, Box } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faQuestion } from "@fortawesome/free-solid-svg-icons"

import DecisionTable from "./decisiontable"
import Variable from "./variables/variable"

const Decision = (props: {
  id: string
  value: any
  handleElement: Function
  handleVariable: Function
  getVariable: Function
}) => {
  return (
    <Card>
      <Card.Header>
        <Card.Header.Icon className="has-text-primary">
          <FontAwesomeIcon icon={faQuestion} size="2x" />
        </Card.Header.Icon>
        <Card.Header.Title>{props?.value?.name}</Card.Header.Title>
      </Card.Header>
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
    </Card>
  )
}

export default Decision
