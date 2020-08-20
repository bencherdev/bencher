import React from "react"
import { Box, Icon, Button } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faPlus } from "@fortawesome/free-solid-svg-icons"

import Parent from "./parent"
import Input from "./input"
import Row from "./row"
import Decision from "./decision"
import Function from "./function"
import Subflow from "./subflow"
import Output from "./output"

const Element = (props: {
  element: any
  handleElement: Function
  getElement: Function
  context: { parent: string; current: string }
  getSubflow: Function
}) => {
  function elementSwitch() {
    switch (props.element.type) {
      case "parent":
        return (
          <Parent
            id={props.element.id}
            value={props.element.value}
            handleElement={props.handleElement}
            context={props.context}
            getSubflow={props.getSubflow}
          />
        )
      case "input":
        return (
          <Input
            id={props.element.id}
            value={props.element.value}
            handleElement={props.handleElement}
            getElement={props.getElement}
            context={props.context}
            getSubflow={props.getSubflow}
          />
        )
      case "row":
        return (
          <Row
            id={props.element.id}
            value={props.element.value}
            handleElement={props.handleElement}
            getElement={props.getElement}
          />
        )
      case "decision":
        return (
          <Decision
            id={props.element.id}
            value={props.element.value}
            handleElement={props.handleElement}
            getElement={props.getElement}
            context={props.context}
          />
        )
      case "function":
        return (
          <Function
            id={props.element.id}
            value={props.element.value}
            handleElement={props.handleElement}
            getElement={props.getElement}
            context={props.context}
          />
        )
      case "subflow":
        return (
          <Subflow
            id={props.element.id}
            value={props.element.value}
            handleElement={props.handleElement}
            getElement={props.getElement}
            context={props.context}
            getSubflow={props.getSubflow}
          />
        )
      case "output":
        return (
          <Output
            id={props.element.id}
            value={props.element.value}
            handleElement={props.handleElement}
            getElement={props.getElement}
            context={props.context}
            getSubflow={props.getSubflow}
          />
        )
      default:
        return <h4>Error: Unknown Element Type</h4>
    }
  }

  return <React.Fragment>{props.element && elementSwitch()}</React.Fragment>
}

export default Element
