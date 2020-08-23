import React from "react"
import { Table } from "react-bulma-components"

import { IconDefinition } from "@fortawesome/free-solid-svg-icons"

import Signature from "./Signature"

const FunctionSignature = (props: {
  id: string
  value: any
  disabled: boolean
  handleVariable: Function
  signature: {
    name: string
    icon: IconDefinition
    handleArgument: Function
    handleInput: Function
  }
}) => {
  // TODO the Signature Table will be a recursive structure
  // This will allow it to handle Complex Tables,
  // that is a Table that contain other Tables
  // as well as Functions.
  return (
    <Signature
      name={props?.signature?.name}
      icon={props?.signature?.icon}
      handleArgument={props?.signature?.handleArgument}
      handleInput={props?.signature?.handleInput}
    >
      <div className="table-container">
        <Table bordered={true} striped={false}>
          <thead>
            <tr>
              <th>Number</th>
            </tr>
          </thead>
        </Table>
      </div>
    </Signature>
  )
}

export default FunctionSignature
