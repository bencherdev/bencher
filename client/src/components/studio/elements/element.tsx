import React, { useState, useEffect } from "react"

import Parent from "./parent"
import Input from "./input"
import Table from "./table"
import Function from "./function"
import Return from "./return"

const Element = (props: {
  element: any
  handleElement: Function
  context: { parent: string; subflow: string }
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
          />
        )
      case "input":
        return (
          <Input
            id={props.element.id}
            value={props.element.value}
            handleElement={props.handleElement}
            context={props.context}
          />
        )
      case "table":
        return (
          <Table
            id={props.element.id}
            value={props.element.value}
            handleElement={props.handleElement}
          />
        )
      case "decision":
        return <p>Decision Table</p>
      case "function":
        return (
          <Function
            id={props.element.id}
            value={props.element.value}
            handleElement={props.handleElement}
          />
        )
      case "subflow":
        return <p>Subflow</p>
      case "return":
        return (
          <Return
            id={props.element.id}
            value={props.element.value}
            handleElement={props.handleElement}
            context={props.context}
          />
        )
      default:
        return <p>Error: Unknown Element Type</p>
    }
  }

  return <>{props.element && elementSwitch()}</>
}

export default Element
