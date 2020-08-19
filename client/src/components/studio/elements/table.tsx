import React from "react"
import { Card, Content } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faTable } from "@fortawesome/free-solid-svg-icons"

import Table from "./variables/table"

const TableElement = (props: {
  id: string
  value: any
  disabled: boolean
  handleElement: Function
}) => {
  return (
    <Card>
      <Card.Header>
        <Card.Header.Icon className="has-text-primary">
          <FontAwesomeIcon icon={faTable} size="2x" />
        </Card.Header.Icon>
        <Card.Header.Title>{props?.value?.name}</Card.Header.Title>
      </Card.Header>
      <Card.Content>
        <Content>
          <Table
            id={props.id}
            value={props.value}
            disabled={props.disabled}
            handleElement={props.handleElement}
          />
        </Content>
      </Card.Content>
    </Card>
  )
}

export default TableElement
