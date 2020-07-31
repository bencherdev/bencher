import React, { ChangeEvent } from "react"
import styled from "styled-components"

const BorderedTable = styled.table`
  border: 1px solid #ddd;
  border-collapse: separate;
  border-left: 0;
  border-radius: 4px;
  border-spacing: 0px;
`

const HoverTr = styled.tr`
  display: table-row;
  vertical-align: inherit;
  border-color: inherit;
  &:first-child {
    border-radius: 4px 0 0 0;
  }
  &:last-child {
    border-radius: 0 0 0 4px;
  }
`

const NameThead = styled.thead`
  display: table-header-group;
  vertical-align: middle;
  border-color: inherit;
  border-collapse: separate;
  &:first-child {
    border-radius: 4px 0 0 0;
  }
  &:last-child {
    border-radius: 0 0 0 4px;
  }
`

const NameTh = styled.th`
  padding: 5px 4px 6px 4px;
  text-align: center;
  vertical-align: top;
  border-left: 1px solid #ddd;
  &:first-child {
    border-radius: 4px 0 0 0;
  }
  &:last-child {
    border-radius: 0 0 0 4px;
  }
`

const BorderedTh = styled.th`
  padding: 5px 4px 6px 4px;
  text-align: left;
  vertical-align: top;
  border-left: 1px solid #ddd;
  &:first-child {
    border-radius: 4px 0 0 0;
  }
  &:last-child {
    border-radius: 0 0 0 4px;
  }
`

const BorderedTd = styled.td`
  padding: 5px 4px 6px 4px;
  text-align: left;
  vertical-align: top;
  border-left: 1px solid #ddd;
  border-top: 1px solid #ddd;
  &:first-child {
    border-radius: 4px 0 0 0;
  }
  &:last-child {
    border-radius: 0 0 0 4px;
  }
`

const Tbody = styled.tbody`
  &:first-child {
    border-radius: 4px 0 0 0;
  }
  &:last-child {
    border-radius: 0 0 0 4px;
  }
`

const CellInput = styled.input`
  border: 0;
`
const Function = (props: {
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
      <NameThead>
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
      </NameThead>
      <Tbody>
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
      </Tbody>
    </BorderedTable>
  )
}

export default Function
