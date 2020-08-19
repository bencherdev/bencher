import React from "react"
import {
  Card,
  Button,
  Content,
  Columns,
  Icon,
  Heading,
} from "react-bulma-components"
import { cloneDeep } from "lodash/lang"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faArrowRight, faPlus } from "@fortawesome/free-solid-svg-icons"

import Variable from "./variables/variable"
import Argument from "./argument"

const Input = (props: {
  id: number
  value: any
  handleElement: Function
  getElement: Function
  context: { parent: string; current: string }
  getSubflow: Function
}) => {
  function handleInput(index: number, elementId: string) {
    if (index >= 0) {
      let functional = cloneDeep(props.value)
      // Check if the index is within the current array
      if (index < props.value.inputs.length) {
        // If the Element ID exists then update the index
        if (elementId && elementId != "") {
          functional.inputs[index] = elementId
          // If the Element ID doesn't exist
          // remove the current Element ID at that index
        } else {
          functional.inputs = functional.inputs.splice(index, 1)
        }
        // If the index is greater and there is an Element ID
        // append it to the inputs array
      } else if (elementId && elementId != "") {
        functional.inputs.push(elementId)
      }
      props.handleElement(props.id, functional)
    }
  }

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
        <Columns centered={true} breakpoint="mobile">
          <Columns.Column size="half">
            <Content className="has-text-centered">
              <Heading size={4}>Input</Heading>
              {props.value?.inputs?.map((elementId: string, index: number) => {
                return (
                  <Argument
                    key={index}
                    element={props?.getElement(elementId)}
                    disabled={false}
                  />
                )
              })}
              <Button
                color="primary"
                outlined={true}
                fullwidth={true}
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
            </Content>
          </Columns.Column>
        </Columns>
      </Card.Content>
      <Card.Footer />
      <Card.Content>
        <Content>
          {props?.value?.inputs?.map((elementId: string, index: number) => {
            return (
              <Variable
                key={index}
                element={props?.getElement(elementId)}
                disabled={false}
                handleElement={props.handleElement}
                getElement={props.getElement}
                context={props.context}
              />
            )
          })}
        </Content>
      </Card.Content>
    </Card>
  )
}

export default Input
