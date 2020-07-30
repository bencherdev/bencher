import React from "react"
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

const TitleTh = styled.th`
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

const Table = (props: { data: any }) => (
  <BorderedTable>
    <thead>
      {props?.data?.title && (
        <tr>
          <TitleTh colSpan={props?.data?.columns?.length}>
            {props?.data?.title}
          </TitleTh>
        </tr>
      )}
      {props?.data?.columns && (
        <tr>
          {props?.data?.columns.map((column: any, index: number) => {
            return <BorderedTh key={index}>{column.name}</BorderedTh>
          })}
        </tr>
      )}
    </thead>
    <tbody>
      {props?.data?.rows &&
        props?.data?.rows.map((row: any, rowIndex: number) => {
          return (
            <HoverTr key={rowIndex}>
              {row.map((cell: any, cellIndex: number) => {
                return (
                  <BorderedTd key={rowIndex + ":" + cellIndex}>
                    {cell}
                  </BorderedTd>
                )
              })}
            </HoverTr>
          )
        })}
    </tbody>
  </BorderedTable>
)

export default Table
