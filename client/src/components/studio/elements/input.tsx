import React from "react"
import { Card, Button, Columns, Icon } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faArrowRight, faPlus } from "@fortawesome/free-solid-svg-icons"

import Variable from "./variables/variable"

const Input = (props: {
  id: string
  value: any
  handleElement: Function
  getElement: Function
  context: { parent: string; current: string }
  getSubflow: Function
}) => {
  return (
    <Card>
      <Card.Header>
        <Card.Header.Icon className="has-text-primary">
          <FontAwesomeIcon icon={faArrowRight} size="2x" />
        </Card.Header.Icon>
        <Card.Header.Title>
          Input to {props?.getSubflow(props?.context?.current)?.name} Subflow
        </Card.Header.Title>
      </Card.Header>
      <Card.Content>
        <Columns centered={true}>
          <Columns.Column>
            {props.value?.inputs?.map((elementId: string, index: number) => {
              return (
                <Variable
                  key={index}
                  element={props?.getElement(elementId)}
                  disabled={{ settings: false, edit: false }}
                  handleElement={props.handleElement}
                  getElement={props.getElement}
                />
              )
            })}
            <Button
              color="primary"
              outlined={true}
              fullwidth={true}
              title="Add input"
              onClick={(event: any) => {
                event.preventDefault()
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
    </Card>
  )
}

export default Input
