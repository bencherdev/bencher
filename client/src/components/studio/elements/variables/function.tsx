import React from "react"
import { Table } from "react-bulma-components"

import Signature from "./signature"

const Function = (props: {
  id: string
  value: any
  disabled: boolean
  handleVariable: Function
}) => {
  // TODO iterate over all signatures for the function's inputs
  return (
    <Signature
      id={props.id}
      value={props.value}
      disabled={props.disabled}
      handleVariable={props.handleVariable}
    />
  )
}

export default Function
