import React, { ChangeEvent } from "react"
import {
  Card,
  Button,
  Content,
  Columns,
  Icon,
  Heading,
  Box,
  Level,
} from "react-bulma-components"
import { cloneDeep } from "lodash/lang"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import {
  faArrowRight,
  faTable,
  faPlus,
  faTimes,
  faPen,
} from "@fortawesome/free-solid-svg-icons"

import Element from "./element"

const Input = (props: {
  id: number
  value: any
  handleElement: Function
  getElement: Function
  context: { parent: string; current: string }
  getSubflowName: Function
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
          Input to {props?.getSubflowName(props?.context?.current)} Subflow
        </Card.Header.Title>
      </Card.Header>
      <Card.Content>
        <Columns centered={true} breakpoint="mobile">
          <Columns.Column size="half">
            <Content className="has-text-centered">
              <Heading size={4}>Input</Heading>
              {props.value?.inputs?.map((elementId: any, index: number) => {
                let element = props?.getElement(elementId)
                return (
                  <Box key={index}>
                    <Level breakpoint="mobile">
                      <Level.Side align="left">
                        <Level.Item>
                          {/* TODO change color to red if there is an input error */}
                          <span className="icon has-text-primary">
                            <FontAwesomeIcon
                              // TODO make this a more advanced switch statement
                              icon={
                                element?.type === "table" ? faTable : faTimes
                              }
                              size="3x"
                            />
                          </span>
                        </Level.Item>
                      </Level.Side>
                      <br />
                      <Level.Side align="center">
                        <Level.Item>
                          <Content className="has-text-centered">
                            <Heading size={5}>
                              {element?.value?.name?.toString()}
                            </Heading>
                          </Content>
                        </Level.Item>
                      </Level.Side>
                      <br />
                      <Level.Side align="right">
                        <Level.Item>
                          <Button
                            size="small"
                            onClick={(event: any) => {
                              console.log("TODO Remove arg")
                            }}
                          >
                            <span className="icon">
                              <FontAwesomeIcon icon={faPen} size="1x" />
                            </span>
                            <span>Edit</span>
                          </Button>
                        </Level.Item>
                      </Level.Side>
                    </Level>
                  </Box>
                )
              })}
              {!props.value?.disabled && (
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
              )}
            </Content>
          </Columns.Column>
        </Columns>
      </Card.Content>
      <Card.Footer />
      <Card.Content>
        <Content>
          {props?.value?.params?.map((_unused: any, index: number) => {
            const elementId = props.value.args?.inputs?.[index]
            return (
              <Element
                key={elementId}
                element={props?.getElement(elementId)}
                handleElement={props.handleElement}
                getElement={props.getElement}
                context={props.context}
                getSubflowName={props.getSubflowName}
              />
            )
          })}
        </Content>
      </Card.Content>
    </Card>
  )
}

export default Input
