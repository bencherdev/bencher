import React, { ChangeEvent } from "react"
import styled from "styled-components"
import { cloneDeep } from "lodash/lang"

const BorderedTable = styled.table`
  border-collapse: collapse;
  border: 1px solid black;
`

const HoverTr = styled.tr`
  hover {
    background-color: #f5f5f5;
  }
`

const NameTh = styled.th`
  border: 1px solid black;
  text-align: center;
  padding: 15px;
`

const BorderedTh = styled.th`
  border: 1px solid black;
  text-align: left;
  padding: 15px;
`

const BorderedTd = styled.td`
  border: 1px solid black;
  padding: 15px;
`

const CellInput = styled.input`
  border: 0;
`

// TODO add table input type validation
const Table = (props: { id: string; value: any; handleElement: Function }) => {
  function handleCell(
    event: ChangeEvent<HTMLInputElement>,
    row: number,
    column: number
  ) {
    if (
      props?.value?.rows?.[row]?.[column] ||
      props?.value?.rows?.[row]?.[column] === ""
    ) {
      let table = cloneDeep(props.value)
      table.rows[row][column] = event.target.value
      props.handleElement(props.id, table)
    }
  }

  return (
    <BorderedTable>
      <thead>
        {props?.value?.name && (
          <tr>
            <NameTh colSpan={props?.value?.columns?.length}>
              {props?.value?.name}
            </NameTh>
          </tr>
        )}
        {props?.value?.columns && (
          <tr>
            {props?.value?.columns.map((column: any, index: number) => {
              return (
                <BorderedTh key={index}>
                  <b>{column?.name}</b>
                  <br />
                  <i>{column?.type}</i>
                </BorderedTh>
              )
            })}
          </tr>
        )}
      </thead>
      <tbody>
        {props?.value?.rows &&
          props?.value?.rows.map((row: any, rowIndex: number) => {
            return (
              <HoverTr key={rowIndex}>
                {row.map((cell: any, columnIndex: number) => {
                  let cellType = props?.value?.columns?.[columnIndex]
                  return (
                    <BorderedTd key={rowIndex + ":" + columnIndex}>
                      <CellInput
                        type={cellType?.toLowerCase}
                        value={cell}
                        onChange={event =>
                          handleCell(event, rowIndex, columnIndex)
                        }
                      />
                    </BorderedTd>
                  )
                })}
              </HoverTr>
            )
          })}
      </tbody>
    </BorderedTable>
  )
}

export default Table
