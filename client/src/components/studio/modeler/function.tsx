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
  text-align: center;
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

  function getArg(index: number, arg: any) {
    return (
      <React.Fragment key={index}>
        {index > 0 && (
          <p>
            <br />
          </p>
        )}
        <b>{arg?.name}</b>
        <br />
        <small>
          <i>{arg?.type}</i>
        </small>
      </React.Fragment>
    )
  }

  function getRow() {
    let params = []
    for (let i = 0; i < props.data.params.length; i++) {
      console.log(props.data.params[i])
      params.push(getArg(i, props.data.params[i]))
    }

    let returns = []
    for (let i = 0; i < props.data.returns.length; i++) {
      console.log(props.data.returns[i])
      returns.push(getArg(i, props.data.returns[i]))
    }

    return (
      <HoverTr>
        <BorderedTd key="args">{params.map(param => param)}</BorderedTd>

        <BorderedTd key="returns">{returns.map(ret => ret)}</BorderedTd>
      </HoverTr>
    )
  }

  return (
    <BorderedTable>
      <NameThead>
        {props?.data?.name && (
          <tr>
            <NameTh colSpan={2}>{props?.data?.name}</NameTh>
          </tr>
        )}
      </NameThead>
      <tbody>{(props?.data?.params || props?.data?.returns) && getRow()}</tbody>
    </BorderedTable>
  )
}

export default Function
