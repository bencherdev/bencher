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

const StaticTable = () => (
  <svg width="100%" height="2000">
    <foreignObject x="10" y="10" width="500" height="500">
      <BorderedTable>
        <thead>
          <tr>
            <TitleTh colSpan={3}>Title</TitleTh>
          </tr>
          <tr>
            <BorderedTh>Lastname</BorderedTh>
            <BorderedTh>Firstname</BorderedTh>
            <BorderedTh>Age</BorderedTh>
          </tr>
        </thead>
        <tbody>
          <HoverTr>
            <BorderedTd rowSpan={2}>Smith</BorderedTd>
            <BorderedTd>Jeff</BorderedTd>
            <BorderedTd>50</BorderedTd>
          </HoverTr>
          <HoverTr>
            <BorderedTd>Jill</BorderedTd>
            <BorderedTd>50</BorderedTd>
          </HoverTr>
          <HoverTr>
            <BorderedTd>Jackson</BorderedTd>
            <BorderedTd>Eve</BorderedTd>
            <BorderedTd>94</BorderedTd>
          </HoverTr>
        </tbody>
      </BorderedTable>
    </foreignObject>
    <circle
      cx="235"
      cy="180"
      r="20"
      stroke="red"
      fill="transparent"
      strokeWidth="5"
    />
  </svg>
)

export default StaticTable
