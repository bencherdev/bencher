import React from "react"

import TableSignature from "./TableSignature"

const FunctionSignature = (props: {
  id: string
  value: any
  disabled: boolean
  handleVariable: Function
}) => {
  // TODO iterate over all signatures for the function's inputs
  return (
    <TableSignature
      id={props.id}
      value={props.value}
      disabled={props.disabled}
      handleVariable={props.handleVariable}
    />
  )
}

export default FunctionSignature
