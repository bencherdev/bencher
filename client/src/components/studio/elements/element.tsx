import React, { useState, useEffect } from "react"

import Parent from "./parent"
import Input from "./input"
import ForeignElement from "./foreignelement"
import Arrow from "./arrow"
import Table from "./table"
import Function from "./function"
import Return from "./return"

const Element = (props: { element: any; handleElement: Function }) => {
  function elementSwitch(element: any) {
    switch (element.type) {
      case "parent":
        return (
          <Parent
            id={element.id}
            value={element.value}
            handleElement={props.handleElement}
          />
        )
      case "input":
        return (
          <Input
            id={element.id}
            value={element.value}
            handleElement={props.handleElement}
          />
        )
      case "table":
        return (
          <Table
            id={element.id}
            value={element.value}
            handleElement={props.handleElement}
          />
        )
      case "decision":
        return <p>Decision Table</p>
      case "function":
        return (
          <Function
            id={element.id}
            value={element.value}
            handleElement={props.handleElement}
          />
        )
      case "subflow":
        return <p>Subflow</p>
      case "return":
        return (
          <Return
            id={element.id}
            value={element.value}
            handleElement={props.handleElement}
          />
        )
      default:
        return <p>Error: Unknown Element Type</p>
    }
  }

  return <>{props.element && elementSwitch(props.element)}</>
}

export default Element
