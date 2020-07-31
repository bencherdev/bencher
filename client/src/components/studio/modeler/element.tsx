import React, { useState, useEffect } from "react"

import Parent from "./parent"
import Input from "./input"
import ForeignElement from "./foreignelement"
import Arrow from "./arrow"
import Table from "./table"
import Function from "./function"
import Return from "./return"

const Element = (props: {
  location: { line: number; position: number }
  prior: any
  element: any
  handleElement: Function
}) => {
  function elementSwitch(element: any) {
    switch (element?.type) {
      case "parent":
        return (
          <Parent
            position={element?.position}
            dimensions={element?.dimensions}
          />
        )
      case "input":
        return (
          <Input
            position={element?.position}
            dimensions={element?.dimensions}
          />
        )
      case "table":
        return (
          <ForeignElement
            position={element?.position}
            dimensions={element?.dimensions}
          >
            <Table
              data={element?.value}
              location={props.location}
              handleElement={props.handleElement}
            />
          </ForeignElement>
        )
      case "decision":
        return (
          <ForeignElement
            position={element?.position}
            dimensions={element?.dimensions}
          >
            <p>Decision Table</p>
          </ForeignElement>
        )
      case "function":
        return (
          <ForeignElement
            position={element?.position}
            dimensions={element?.dimensions}
          >
            <Function
              data={element?.value}
              location={props.location}
              handleElement={props.handleElement}
            />
          </ForeignElement>
        )
      case "subflow":
        return <p>Subflow</p>
      case "return":
        return (
          <Return
            position={element?.position}
            dimensions={element?.dimensions}
          />
        )
      default:
        return (
          <ForeignElement
            position={element?.position}
            dimensions={element?.dimensions}
          >
            <p>Error: Unknown Element Type</p>
          </ForeignElement>
        )
    }
  }

  return (
    <g>
      {elementSwitch(props?.element)}
      {props?.prior && (
        <Arrow source={props?.prior} destination={props?.element} />
      )}
    </g>
  )
}

export default Element
