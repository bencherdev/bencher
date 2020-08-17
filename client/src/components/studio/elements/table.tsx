import React, { ChangeEvent } from "react"
import { Table } from "react-bulma-components"
import { cloneDeep } from "lodash/lang"

import ContentEditable from "../../utils/contenteditable"
import sanitize from "../../utils/sanitize"

import typeSelect from "./typeselect"
import Select from "../../utils/forms/select"

// TODO add table input type validation
const TableElement = (props: {
  id: string
  value: any
  handleElement: Function
}) => {
  function handleName(event: ChangeEvent<HTMLInputElement>) {
    let table = cloneDeep(props.value)
    table.name = sanitize.toText(event.target.value)
    props.handleElement(props.id, table)
  }

  function handleColumn(event: ChangeEvent<HTMLInputElement>, column: number) {
    if (
      props.value?.columns?.[column]?.name ||
      props.value?.columns?.[column]?.name === ""
    ) {
      let table = cloneDeep(props.value)
      table.columns[column].name = sanitize.toText(event.target.value)
      props.handleElement(props.id, table)
    }
  }

  function handleType(event: ChangeEvent<HTMLInputElement>, column: number) {
    if (
      props.value?.columns?.[column]?.type ||
      props.value?.columns?.[column]?.type === ""
    ) {
      let table = cloneDeep(props.value)
      table.columns[column].type = event.target.value
      props.handleElement(props.id, table)
    }
  }

  function handleCell(
    event: ChangeEvent<HTMLInputElement>,
    row: number,
    column: number
  ) {
    if (
      props.value?.rows?.[row]?.[column] ||
      props.value?.rows?.[row]?.[column] === ""
    ) {
      let table = cloneDeep(props.value)
      table.rows[row][column] = sanitize.toText(event.target.value)
      props.handleElement(props.id, table)
    }
  }

  return (
    <div className="table-container">
      <Table bordered={true} striped={false}>
        <thead>
          <tr>
            <ContentEditable
              html={sanitize.toHtml(props?.value?.name.toString())}
              disabled={props.value?.disabled}
              onChange={(event: any) => handleName(event)}
              tagName="th"
              // TODO change color to red if there is an input error
              style={{
                colSpan: `${props.value?.columns?.length}`,
                textAlign: "center",
                outlineColor: "#009933",
              }}
            />
          </tr>
          {props?.value?.columns && (
            <tr>
              {props?.value?.columns.map((column: any, index: number) => {
                return (
                  <ContentEditable
                    key={index}
                    html={sanitize.toHtml(column?.name?.toString())}
                    disabled={props.value?.disabled}
                    onChange={(event: any) => handleColumn(event, index)}
                    tagName="th"
                    // TODO change color to red if there is an input error
                    style={{ outlineColor: "#009933" }}
                  />
                )
              })}
            </tr>
          )}
          {props.value?.columns && (
            <tr>
              {props.value?.columns.map((column: any, index: number) => {
                return (
                  <th key={index}>
                    <Select
                      disabled={props.value?.disabled}
                      column={index}
                      selected={column?.type}
                      config={typeSelect}
                      handleType={handleType}
                    />
                  </th>
                )
              })}
            </tr>
          )}
        </thead>
        <tbody>
          {props.value?.rows &&
            props.value?.rows?.map((row: any, rowIndex: number) => {
              return (
                <tr key={rowIndex}>
                  {row.map((cell: string, columnIndex: number) => {
                    return (
                      <ContentEditable
                        key={rowIndex + ":" + columnIndex}
                        html={sanitize.toHtml(cell?.toString())}
                        disabled={props.value?.disabled}
                        onChange={(event: any) =>
                          handleCell(event, rowIndex, columnIndex)
                        }
                        tagName="td"
                        // TODO change color to red if there is an input error
                        style={{ outlineColor: "#009933" }}
                      />
                    )
                  })}
                </tr>
              )
            })}
        </tbody>
      </Table>
    </div>
  )
}

export default TableElement
