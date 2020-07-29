import React, { useState, useEffect } from "react"

import NewElement from "./newelement"
import ForeignElement from "./foreignelement"
import Arrow from "./arrow"
import Table from "./table"

const Element = (props: { prior: any; element: any }) => {
  function elementSwitch(element: any) {
    switch (element?.type) {
      case "new":
        return (
          <NewElement
            position={element?.position}
            dimensions={element?.dimensions}
          />
        )
      case "parent":
        return <p>Parent Flow</p>
      case "table":
        return (
          <ForeignElement
            position={element?.position}
            dimensions={element?.dimensions}
          >
            <Table />
          </ForeignElement>
        )
      case "decision":
        return <p>Decision Table</p>
      case "function":
        return <p>Function</p>
      case "subflow":
        return <p>Subflow</p>
      case "return":
        return <p>Return</p>
      default:
        return <p>Error: Unknown Element Type</p>
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
