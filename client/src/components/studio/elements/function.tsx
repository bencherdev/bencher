import React, { ChangeEvent } from "react"
import styled from "styled-components"

const BorderedTable = styled.table`
  border: 1px solid black;
  border-collapse: separate;
  border-left: 0;
  border-radius: 4px;
`

const NameThead = styled.thead`
  text-align: center;
  vertical-align: middle;
`

const NameTh = styled.th`
  padding: 15px;
  border-left: 1px solid black;
`

const BorderedTd = styled.td`
  padding: 15px;
  text-align: left;
  vertical-align: top;
  border-left: 1px solid black;
  border-top: 1px solid black;
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
      params.push(getArg(i, props.data.params[i]))
    }

    let returns = []
    for (let i = 0; i < props.data.returns.length; i++) {
      returns.push(getArg(i, props.data.returns[i]))
    }

    return (
      <tr>
        <BorderedTd key="args">{params.map(param => param)}</BorderedTd>

        <BorderedTd key="returns">{returns.map(ret => ret)}</BorderedTd>
      </tr>
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
