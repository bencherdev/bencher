import React, { ChangeEvent } from "react"
import { Table, Button, Icon } from "react-bulma-components"
import { cloneDeep } from "lodash/lang"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faQuestionCircle } from "@fortawesome/free-solid-svg-icons"

import ContentEditable from "../../../utils/contenteditable"
import sanitize from "../../../utils/sanitize"

import typeSelect from "../variables/tabular/typeselect"
// todo either create a new Select that can update *another* Element
// or add a tableID as part of the props to the current Select component
import Select from "../../../utils/forms/select"

const DecisionTable = (props: {
  id: string
  value: any
  disabled: boolean
  handleElement: Function
  handleVariable: Function
  getVariable: Function
}) => {
  // TODO add event.preventDefault()
  // TODO switch callbacks to closures for the selects

  function handleName(event: ChangeEvent<HTMLInputElement>) {
    let table = cloneDeep(props.value)
    table.name = sanitize.toText(event.target.value)
    props.handleElement(props.id, table)
  }

  function handleInputTable(
    event: ChangeEvent<HTMLInputElement>,
    column: number,
    tableId: string
  ) {
    // props.handleElement()
    console.log("TODO set the input table to a new Table ID")
  }

  function handleInputColumn(
    event: ChangeEvent<HTMLInputElement>,
    column: number,
    tableId: string,
    columnId: string
  ) {
    // props.handleElement()
    console.log(
      "TODO set the input column to a new Column ID for the input Table"
    )
  }

  function handleOutputTableName(
    event: ChangeEvent<HTMLInputElement>,
    column: number,
    tableId: string
  ) {
    // props.handleVariable()
    console.log("TODO update the output table with the new value")
  }

  function handleOutputTableColumnName(
    event: ChangeEvent<HTMLInputElement>,
    column: number,
    tableId: string,
    columnId: string
  ) {
    // props.handleVariable()
    console.log("TODO update the output table with the new value")
  }

  function handleOutputTableColumnType(
    event: ChangeEvent<HTMLInputElement>,
    column: number
  ) {
    const headerId = props.value?.columns?.outputs?.[column]
    if (
      headerId &&
      (props.value?.headers?.outputs?.[headerId]?.type ||
        props.value?.headers?.outputs?.[headerId]?.type === "")
    ) {
      let table = cloneDeep(props.value)
      table.headers.outputs[headerId].type = event.target.value
      props.handleVariable(props.id, table)
    }
  }

  function handleConditions(
    event: ChangeEvent<HTMLInputElement>,
    column: number
  ) {
    const headerId = props.value?.columns?.inputs?.[column]
    let table = cloneDeep(props.value)
    table.headers.inputs[headerId].conditions = sanitize.toText(
      event.target.value
    )
    props.handleElement(props.id, table)
  }

  function handleCell(
    event: ChangeEvent<HTMLInputElement>,
    rowIndex: number,
    io: string,
    columnId: string
  ) {
    if (
      props.value?.rows?.[rowIndex]?.[io]?.[columnId] ||
      props.value?.rows?.[rowIndex]?.[io]?.[columnId] === ""
    ) {
      let table = cloneDeep(props.value)
      table.rows[rowIndex][io][columnId] = sanitize.toText(event.target.value)
      props.handleElement(props.id, table)
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
              // TODO change color to red if there is an input error
              colSpan={
                props.value?.columns?.inputs?.length +
                props.value?.columns?.outputs?.length
              }
              style={{
                textAlign: "center",
                outlineColor: "#009933",
              }}
            />
          </tr>
          <tr>
            <th
              colSpan={props.value?.columns?.inputs?.length}
              style={{
                textAlign: "center",
              }}
            >
              Input
            </th>
            <th
              colSpan={props.value?.columns?.outputs?.length}
              style={{
                textAlign: "center",
              }}
            >
              Output
            </th>
          </tr>
          <tr>
            {props.value?.columns?.inputs?.map(
              (headerId: any, index: number) => {
                const table = props.getVariable(
                  props.value?.headers?.inputs?.[headerId]?.table
                )
                const tableName = table?.value?.name?.toString()
                return (
                  <th key={index}>
                    <Select
                      name="Input Table Name"
                      disabled={props.disabled}
                      column={index}
                      selected={tableName}
                      // TODO Create a dynmic select for all possible tables
                      config={{
                        options: [
                          {
                            value: tableName,
                            option: tableName,
                          },
                        ],
                      }}
                      handleSelect={handleInputTable}
                    />
                  </th>
                )
              }
            )}
            {props.value?.columns?.outputs?.map(
              (headerId: any, index: number) => {
                const table = props.getVariable(
                  props.value?.headers?.outputs?.[headerId]?.table
                )
                return (
                  <ContentEditable
                    key={index}
                    html={sanitize.toHtml(table?.value?.name?.toString())}
                    disabled={props.disabled}
                    onChange={(event: any) =>
                      handleOutputTableName(event, index, table?.id)
                    }
                    tagName="th"
                    // TODO change color to red if there is an input error
                    style={{ outlineColor: "#009933" }}
                  />
                )
              }
            )}
          </tr>
          <tr>
            {props.value?.columns?.inputs?.map(
              (headerId: any, index: number) => {
                const table = props.getVariable(
                  props.value?.headers?.inputs?.[headerId]?.table
                )
                const column =
                  table?.value?.headers?.[
                    props.value?.headers?.inputs?.[headerId]?.column
                  ]
                return (
                  <th key={index}>
                    <Select
                      name="Column Name from Input Table"
                      disabled={props.disabled}
                      column={index}
                      selected={column?.name}
                      // TODO turn this into a dynamic select
                      // for the selected Table's columns
                      config={{
                        options: [
                          {
                            value: column?.name,
                            option: column?.name,
                          },
                        ],
                      }}
                      handleSelect={handleInputColumn}
                    />
                  </th>
                )
              }
            )}
            {props.value?.columns?.outputs?.map(
              (headerId: any, index: number) => {
                const table = props.getVariable(
                  props.value?.headers?.outputs?.[headerId]?.table
                )
                const tableColumn =
                  table?.value?.headers?.[
                    props.value?.headers?.outputs?.[headerId]?.column
                  ]
                return (
                  <ContentEditable
                    key={index}
                    html={sanitize.toHtml(tableColumn?.name)}
                    disabled={props.disabled}
                    onChange={(event: any) =>
                      handleOutputTableColumnName(
                        event,
                        index,
                        table?.id,
                        tableColumn?.id
                      )
                    }
                    tagName="th"
                    // TODO change color to red if there is an input error
                    style={{ outlineColor: "#009933" }}
                  />
                )
              }
            )}
          </tr>
          <tr>
            {props.value?.columns?.inputs?.map(
              (headerId: any, index: number) => {
                const tableConditions =
                  props.value?.headers?.inputs?.[headerId]?.conditions
                return (
                  <ContentEditable
                    key={index}
                    html={sanitize.toHtml(tableConditions)}
                    disabled={props.disabled}
                    onChange={(event: any) => handleConditions(event, index)}
                    tagName="th"
                    // TODO change color to red if there is an input error
                    style={{ outlineColor: "#009933" }}
                  />
                )
              }
            )}
            {props.value?.columns?.outputs?.map(
              (headerId: any, index: number) => {
                const table = props.getVariable(
                  props.value?.headers?.outputs?.[headerId]?.table
                )
                const tableColumn =
                  table?.value?.headers?.[
                    props.value?.headers?.outputs?.[headerId]?.column
                  ]
                return (
                  <th key={index}>
                    <Select
                      name="Output Column Type"
                      disabled={props.disabled}
                      column={index}
                      selected={tableColumn?.type}
                      config={typeSelect}
                      handleSelect={handleOutputTableColumnType}
                    />
                  </th>
                )
              }
            )}
          </tr>
        </thead>
        <tbody>
          {props.value?.rows?.map((row: any, rowIndex: number) => {
            return (
              <tr key={rowIndex}>
                {props.value?.columns?.inputs?.map(
                  (columnId: string, columnIndex: number) => {
                    return (
                      <ContentEditable
                        key={rowIndex + ":" + columnIndex}
                        html={sanitize.toHtml(
                          row?.inputs?.[columnId]?.toString()
                        )}
                        disabled={props.disabled}
                        onChange={(event: any) =>
                          handleCell(event, rowIndex, "inputs", columnId)
                        }
                        tagName="td"
                        // TODO change color to red if there is an input error
                        style={{ outlineColor: "#009933" }}
                      />
                    )
                  }
                )}
                {props.value?.columns?.outputs?.map(
                  (columnId: string, columnIndex: number) => {
                    const output = row?.outputs?.[columnId]?.toString()
                    if (output.startsWith("subflow://")) {
                      if (columnIndex == 0) {
                        const subflowId = output.slice("subflow://".length)
                        return (
                          <td
                            key={rowIndex + ":" + columnIndex}
                            colSpan={props.value?.columns?.outputs?.length}
                          >
                            <Button
                              color="primary"
                              outlined={true}
                              size="small"
                              fullwidth={true}
                              title="Settings"
                              onClick={(event: any) => {
                                event.preventDefault()
                                console.log(
                                  "TODO redirect to Decision Subflow",
                                  subflowId
                                )
                              }}
                            >
                              <Icon>
                                <FontAwesomeIcon
                                  icon={faQuestionCircle}
                                  size="1x"
                                />
                              </Icon>
                              <span>Decision Subflow Case #{rowIndex + 1}</span>
                            </Button>
                          </td>
                        )
                      }
                      return
                    }
                    return (
                      <ContentEditable
                        key={rowIndex + ":" + columnIndex}
                        html={sanitize.toHtml(output)}
                        disabled={props.disabled}
                        onChange={(event: any) =>
                          handleCell(event, rowIndex, "outputs", columnId)
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

export default DecisionTable
