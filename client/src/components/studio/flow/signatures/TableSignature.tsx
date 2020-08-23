import React from "react"
import { Table } from "react-bulma-components"

import { faTable } from "@fortawesome/free-solid-svg-icons"

import Signature from "./Signature"

const TableSignature = (props: {
  value: {
    name: string
    type: string
    arguments: { inputs: [string]; outputs: [string] }
  }
  handleArgument: Function
}) => {
  // TODO the Signature Table will be a recursive structure
  // This will allow it to handle Complex Tables,
  // that is a Table that contain other Tables
  // as well as Functions.
  return (
    <Signature
      name={props?.value?.name}
      icon={faTable}
      handleArgument={props?.handleArgument}
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

export default TableSignature
