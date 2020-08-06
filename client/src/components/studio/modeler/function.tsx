import React, { ChangeEvent } from "react"
import styled from "styled-components"

const BorderedTable = styled.table`
  border: 1px solid black;
  border-collapse: separate;
  border-left: 0;
  border-radius: 4px;
  border-spacing: 0px;
`

const HoverTr = styled.tr`
  display: table-row;
  vertical-align: inherit;
  border-color: inherit;
`

const NameThead = styled.thead`
  display: table-header-group;
  vertical-align: middle;
  border-color: inherit;
  border-collapse: separate;
`

const NameTh = styled.th`
  padding: 5px 4px 6px 4px;
  text-align: center;
  vertical-align: top;
  border-left: 1px solid black;
`

const BorderedTh = styled.th`
  padding: 5px 4px 6px 4px;
  text-align: left;
  vertical-align: top;
  border-left: 1px solid black;
  border-top: 1px solid black;
`

const BorderedTd = styled.td`
  padding: 5px 4px 6px 4px;
  text-align: left;
  vertical-align: top;
  border-left: 1px solid black;
  border-top: 1px solid black;
`

const CellInput = styled.input`
  border: 0;
`
const Function = (props: {
  location: { line: number; position: number }
  data: any
  handleElement: Function
}) => {
  function handleArgs() {}
  function handleReturns() {}

  return (
    <BorderedTable>
      <NameThead>
        {props?.data?.name && (
          <tr>
            <NameTh colSpan={2}>{props?.data?.name}</NameTh>
          </tr>
        )}
        {props?.data?.params && (
          <tr>
            {props?.data?.params.map((param: any, index: number) => {
              return (
                <BorderedTh key={index}>
                  <b>{param?.name}</b>
                  <br />
                  <i>{param?.type}</i>
                </BorderedTh>
              )
            })}
          </tr>
        )}
      </NameThead>
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

export default Function
