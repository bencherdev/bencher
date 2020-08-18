import React, { ChangeEvent } from "react"
import { Card, Button, Content, Columns, Icon } from "react-bulma-components"
import { cloneDeep } from "lodash/lang"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import {
  faArrowRight,
  faTable,
  faPlus,
} from "@fortawesome/free-solid-svg-icons"

import Element from "./element"

import ContentEditable from "../../utils/contenteditable"
import sanitize from "../../utils/sanitize"

const Input = (props: {
  id: number
  value: any
  handleElement: Function
  getElement: Function
  context: { parent: string; current: string }
  getSubflowName: Function
}) => {
  function handleParam(event: ChangeEvent<HTMLInputElement>, param: number) {
    if (
      props.value?.params?.[param]?.name ||
      props.value?.params?.[param]?.name === ""
    ) {
      let table = cloneDeep(props.value)
      table.params[param].name = sanitize.toText(event.target.value)
      props.handleElement(props.id, table)
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
          <Columns.Column>
            <Content className="has-text-centered">
              {props.value?.params?.map((param: any, index: number) => {
                const input = props.value.args?.inputs?.[index]
                return (
                  <div key={index}>
                    <ContentEditable
                      html={sanitize.toHtml(param?.name?.toString())}
                      disabled={props.value?.disabled}
                      onChange={(event: any) => handleParam(event, index)}
                      tagName="h4"
                      // TODO change color to red if there is an input error
                      style={{
                        textAlign: "center",
                        outlineColor: "#009933",
                      }}
                    />
                    {/* TODO on click redirect to ref for input decleration
                  if it is from elsewhere */}
                    <span className="icon has-text-primary">
                      <FontAwesomeIcon icon={faTable} size="3x" />
                    </span>
                    <h5>{props.getElement(input)?.value?.name}</h5>
                  </div>
                )
              })}
              {!props.value?.disabled && (
                <Button
                  color="primary"
                  outlined={true}
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
