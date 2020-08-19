import React from "react"
import { Card, Heading, Button, Content, Columns } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faQuestion } from "@fortawesome/free-solid-svg-icons"

import Argument from "./argument"
import Variable from "./variables/variable"

const Decision = (props: {
  id: number
  value: any
  handleElement: Function
  getElement: Function
  context: { parent: string; current: string }
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
        <Columns centered={true} breakpoint="mobile">
          <Columns.Column size="half">
            <Content className="has-text-centered">
              <Heading size={4}>Input</Heading>
              {props?.value?.inputs?.map((elementId: string, index: number) => {
                return (
                  <Argument
                    key={index}
                    element={props.getElement(elementId)}
                    disabled={false}
                  />
                )
              })}
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
                      disabled={true}
                    />
                  )
                }
              )}
            </Content>
          </Columns.Column>
        </Columns>
      </Card.Content>
      <Card.Footer />
      <Card.Content>
        <Content>
          {props?.value?.outputs?.map((elementId: string, index: number) => {
            return (
              <Variable
                key={index}
                element={props?.getElement(elementId)}
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

export default Decision
