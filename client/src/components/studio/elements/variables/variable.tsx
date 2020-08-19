import React from "react"
import { Box, Icon, Button } from "react-bulma-components"

import Table from "./table"
import Signature from "./signature"

const Variable = (props: {
  element: any
  disabled: boolean
  handleElement: Function
  getElement: Function
  context: { parent: string; current: string }
}) => {
  function variableSwitch() {
    switch (props.element.type) {
      case "table":
        return (
          <Table
            id={props.element.id}
            value={props.element.value}
            disabled={props.disabled}
            handleElement={props.handleElement}
          />
        )
      case "signature":
        return (
          <Signature
            id={props.element.id}
            value={props.element.value}
            disabled={props.disabled}
            handleElement={props.handleElement}
          />
        )
      default:
        return <p>Error: Unknown Element Type</p>
    }
  }

  return <React.Fragment>{props.element && variableSwitch()}</React.Fragment>
}

export default Variable
