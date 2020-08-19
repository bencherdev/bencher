import React, { useState } from "react"
import {
  Card,
  Heading,
  Button,
  Content,
  Columns,
  Icon,
} from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faCircle, faPlus } from "@fortawesome/free-solid-svg-icons"

import Argument from "./argument"
import Variable from "./variables/variable"

const Subflow = (props: {
  id: number
  value: any
  handleElement: Function
  getElement: Function
  context: { parent: string; current: string }
  // TODO change this call to just getSubflow
  getSubflow: Function
}) => {
  const [subflow, setSubflow] = useState(props.getSubflow(props?.value?.id))

  function getElement(id: string): any {
    // TODO use this to get the Elements in the Sublfow
    return subflow?.elements?.[id]
  }

  function handleSubflow() {
    // TODO need to be able to change both ourselves
    // and the underlying subflow from calls here
    // getElement(subflow?.input)?.value?.inputs?
    // getElement(subflow?.output)?.value?.outputs?
  }

  return (
    <Card>
      <Card.Header>
        <Card.Header.Icon className="has-text-primary">
          <FontAwesomeIcon icon={faCircle} size="2x" />
        </Card.Header.Icon>
        <Card.Header.Title>{subflow?.name}</Card.Header.Title>
      </Card.Header>
      <Card.Content>
        <Columns centered={true} breakpoint="mobile">
          <Columns.Column size="half">
            <Content className="has-text-centered">
              <Heading size={4}>Input</Heading>
              {props?.value?.inputs?.map((elementId: string, index: number) => {
                // TODO Figure out the distinction between
                // variables that are passed into the Input for a Subflow
                // and variables that are set from the Input for a Subflow
                return (
                  <Argument
                    key={index}
                    element={props.getElement(elementId)}
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
          <Columns.Column size="half">
            <Content className="has-text-centered">
              <Heading size={4}>Output</Heading>
              {props?.value?.outputs?.map(
                (elementId: string, index: number) => {
                  return (
                    <Argument
                      key={index}
                      element={props.getElement(elementId)}
                      disabled={false}
                    />
                  )
                }
              )}
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
            </Content>
          </Columns.Column>
        </Columns>
      </Card.Content>
      <Card.Footer />
      <Card.Content>
        <Content>
          {props?.value?.outputs?.map((elementId: string, index: number) => {
            console.log(elementId)
            return (
              <Variable
                key={index}
                element={props.getElement(elementId)}
                disabled={true}
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

export default Subflow
