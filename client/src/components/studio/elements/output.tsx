import React from "react"
import { Card, Columns, Button, Icon } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faArrowLeft, faPlus } from "@fortawesome/free-solid-svg-icons"

import Variable from "./variables/variable"

const Output = (props: {
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
          <FontAwesomeIcon icon={faArrowLeft} size="2x" />
        </Card.Header.Icon>
        <Card.Header.Title>
          Output from {props?.getSubflow(props?.context?.current)?.name} Subflow
        </Card.Header.Title>
      </Card.Header>
      <Card.Content>
        <Columns centered={true} breakpoint="mobile">
          <Columns.Column>
            {props?.value?.outputs?.map((elementId: string, index: number) => {
              return (
                <Variable
                  key={index}
                  element={props?.getElement(elementId)}
                  disabled={{ settings: false, edit: true }}
                  handleElement={props.handleElement}
                  getElement={props.getElement}
                />
              )
            })}
            <Button
              color="primary"
              outlined={true}
              fullwidth={true}
              onClick={(event: any) => {
                event.preventDefault()
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
