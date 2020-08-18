import React from "react"
import {
  Card,
  Content,
  Columns,
  Heading,
  Button,
  Icon,
} from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faArrowLeft, faPlus } from "@fortawesome/free-solid-svg-icons"

import Argument from "./argument"
import Variable from "./variables/variable"

const Output = (props: {
  id: number
  value: any
  handleElement: Function
  getElement: Function
  context: { parent: string; current: string }
  getSubflowName: Function
}) => {
  return (
    <Card>
      <Card.Header>
        <Card.Header.Icon className="has-text-primary">
          <FontAwesomeIcon icon={faArrowLeft} size="2x" />
        </Card.Header.Icon>
        <Card.Header.Title>
          Output from {props?.getSubflowName(props?.context?.current)} Subflow
        </Card.Header.Title>
      </Card.Header>
      <Card.Content>
        <Columns centered={true} breakpoint="mobile">
          <Columns.Column>
            <Content className="has-text-centered">
              <Heading size={4}>Output</Heading>
              {props?.value?.returns?.map((ret: any, index: number) => {
                const output = props.value.args?.outputs?.[index]
                return (
                  <Argument
                    key={index}
                    element={props.getElement(output)}
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
          {props?.value?.returns?.map((_unused: any, index: number) => {
            const elementId = props.value.args?.outputs?.[index]
            return (
              <Variable
                key={elementId}
                element={props?.getElement(elementId)}
                disabled={true}
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

export default Output
