import React from "react"
import { Table } from "react-bulma-components"

const TableSignature = (props: {
  id: string
  value: any
  disabled: boolean
  handleVariable: Function
}) => {
  // TODO the Signature Table will be a recursive structure
  // This will allow it to handle Complex Tables,
  // that is a Table that contain other Tables
  // as well as Functions.
  return (
    <div className="table-container">
      <Table bordered={true} striped={false}>
        <thead>
          <tr>
            <th>Number</th>
          </tr>
        </thead>
      </Table>
    </div>
  )
}

export default TableSignature
