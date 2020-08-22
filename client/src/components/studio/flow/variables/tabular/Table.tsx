import React, { ChangeEvent } from "react"
import { Table } from "react-bulma-components"
import { cloneDeep } from "lodash/lang"

import ContentEditable from "../../../../utils/contenteditable"
import sanitize from "../../../../utils/sanitize"

import typeSelect from "./typeselect"
import Select from "../../../../utils/forms/select"

// TODO add table input type validation
const IOTable = (props: {
  id: string
  value: any
  disabled: boolean
  handleVariable: Function
}) => {
  function handleName(event: ChangeEvent<HTMLInputElement>) {
    let table = cloneDeep(props.value)
    table.name = sanitize.toText(event.target.value)
    props.handleVariable(props.id, table)
  }

  function handleColumn(event: ChangeEvent<HTMLInputElement>, column: number) {
    const headerId = props.value?.columns?.[column]
    if (
      headerId &&
      (props.value?.headers?.[headerId]?.name ||
        props.value?.headers?.[headerId]?.name === "")
    ) {
      let table = cloneDeep(props.value)
      table.headers[headerId].name = sanitize.toText(event.target.value)
      props.handleVariable(props.id, table)
    }
  }

  function handleType(event: ChangeEvent<HTMLInputElement>, column: number) {
    const headerId = props.value?.columns?.[column]
    if (
      headerId &&
      (props.value?.headers?.[headerId]?.type ||
        props.value?.headers?.[headerId]?.type === "")
    ) {
      let table = cloneDeep(props.value)
      table.headers[headerId].type = event.target.value
      props.handleVariable(props.id, table)
    }
  }

  function handleCell(
    event: ChangeEvent<HTMLInputElement>,
    rowIndex: number,
    columnId: string
  ) {
    if (
      props.value?.rows?.[rowIndex]?.[columnId] ||
      props.value?.rows?.[rowIndex]?.[columnId] === ""
    ) {
      let table = cloneDeep(props.value)
      table.rows[rowIndex][columnId] = sanitize.toText(event.target.value)
      props.handleVariable(props.id, table)
    }
  }

  return (
    <div className="table-container">
      <Table bordered={true} striped={false}>
        <thead>
          <tr>
            <ContentEditable
              html={sanitize.toHtml(props.value?.name?.toString())}
              disabled={props.disabled}
              onChange={(event: any) => handleName(event)}
              tagName="th"
              colSpan={props.value?.columns?.length}
              // TODO change color to red if there is an input error
              style={{
                textAlign: "center",
                outlineColor: "#009933",
              }}
            />
          </tr>
          <tr>
            {props.value?.columns?.map((headerId: any, index: number) => {
              const header = props.value?.headers?.[headerId]
              return (
                <ContentEditable
                  key={index}
                  html={sanitize.toHtml(header?.name?.toString())}
                  disabled={props.disabled}
                  onChange={(event: any) => handleColumn(event, index)}
                  tagName="th"
                  // TODO change color to red if there is an input error
                  style={{ outlineColor: "#009933" }}
                />
              )
            })}
          </tr>
          <tr>
            {props.value?.columns?.map((headerId: any, index: number) => {
              const header = props.value?.headers?.[headerId]
              return (
                <th key={index}>
                  <Select
                    name="Column Type Select"
                    disabled={props.disabled}
                    column={index}
                    selected={header?.type}
                    config={typeSelect}
                    handleSelect={handleType}
                  />
                </th>
              )
            })}
          </tr>
        </thead>
        <tbody>
          {props.value?.rows?.map((row: any, rowIndex: number) => {
            return (
              <tr key={rowIndex}>
                {props.value?.columns?.map(
                  (columnId: string, columnIndex: number) => {
                    return (
                      <ContentEditable
                        key={rowIndex + ":" + columnIndex}
                        html={sanitize.toHtml(row?.[columnId]?.toString())}
                        disabled={props.disabled}
                        onChange={(event: any) =>
                          handleCell(event, rowIndex, columnId)
                        }
                        tagName="td"
                        // TODO change color to red if there is an input error
                        style={{ outlineColor: "#009933" }}
                      />
                    )
                  }
                )}
              </tr>
            )
          })}
        </tbody>
      </Table>
    </div>
  )
}

export default IOTable
