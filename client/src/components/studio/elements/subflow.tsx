import React from "react"
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
  getSubflowName: Function
}) => {
  return (
    <Card>
      <Card.Header>
        <Card.Header.Icon className="has-text-primary">
          <FontAwesomeIcon icon={faCircle} size="2x" />
        </Card.Header.Icon>
        <Card.Header.Title>
          {props.getSubflowName(props?.value?.id)}
        </Card.Header.Title>
      </Card.Header>
      <Card.Content>
        <Columns centered={true} breakpoint="mobile">
          <Columns.Column size="half">
            <Content className="has-text-centered">
              <Heading size={4}>Input</Heading>
              {props?.value?.params?.map((param: any, index: number) => {
                const input = props.value.args?.inputs?.[index]
                return (
                  <Argument
                    key={index}
                    element={props.getElement(input)}
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
              {props?.value?.returns?.map((ret: any, index: number) => {
                const output = props.value.args?.outputs?.[index]
                return (
                  <Argument
                    key={index}
                    element={props.getElement(output)}
                    disabled={true}
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

export default Subflow
