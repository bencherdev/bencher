import React from "react"
import { Table } from "react-bulma-components"

const TableSignature = (props: {
  id: string
  value: any
  disabled: boolean
  handleVariable: Function
}) => {
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
