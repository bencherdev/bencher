import React, { ChangeEvent } from "react"
import styled from "styled-components"

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
const Table = (props: {
  location: { line: number; position: number }
  data: any
  handleElement: Function
}) => {
  function handleCell(
    event: ChangeEvent<HTMLInputElement>,
    row: number,
    column: number
  ) {
    if (
      props?.data?.rows?.[row]?.[column] ||
      props?.data?.rows?.[row]?.[column] === ""
    ) {
      let table = JSON.parse(JSON.stringify(props.data))
      table.rows[row][column] = event.target.value
      props.handleElement(props.location, table)
    }
  }

  return (
    <BorderedTable>
      <thead>
        {props?.data?.name && (
          <tr>
            <NameTh colSpan={props?.data?.columns?.length}>
              {props?.data?.name}
            </NameTh>
          </tr>
        )}
        {props?.data?.columns && (
          <tr>
            {props?.data?.columns.map((column: any, index: number) => {
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
        {props?.data?.rows &&
          props?.data?.rows.map((row: any, rowIndex: number) => {
            return (
              <HoverTr key={rowIndex}>
                {row.map((cell: any, columnIndex: number) => {
                  let cellType = props?.data?.columns?.[columnIndex]
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
