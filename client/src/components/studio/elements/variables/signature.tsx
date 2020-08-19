import React from "react"
import { Table } from "react-bulma-components"

const Signature = (props: {
  id: string
  value: any
  disabled: boolean
  handleElement: Function
}) => {
  return (
    <div className="table-container">
      <Table bordered={true} striped={false}>
        <thead>
          <tr>
            <th
              style={{
                colSpan: "1",
                textAlign: "center",
              }}
            >
              Function Signature
            </th>
          </tr>
          <tr>
            <th>Some Input</th>
            <th>Some Output</th>
          </tr>
        </thead>
        <tbody>
          <td>Table Template</td>
          <td>Table Template</td>
        </tbody>
      </Table>
    </div>
  )
}

export default Signature
