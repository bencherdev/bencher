import React from "react"

import Table from "./Table"

const Row = (props: {
  id: string
  value: any
  disabled: boolean
  handleVariable: Function
}) => {
  // TODO create a Row type wrapper around a Table
  // it should only allow for a single row to be created
  return (
    <Table
      id={props.id}
      value={props.value}
      disabled={props.disabled}
      handleVariable={props.handleVariable}
    />
  )
}

export default Row
