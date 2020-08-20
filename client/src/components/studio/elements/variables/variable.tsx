import React from "react"
import { Box, Button, Icon } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faCog } from "@fortawesome/free-solid-svg-icons"

import Table from "./table"
import Function from "./function"
import Row from "./row"

const Variable = (props: {
  variable: any
  disabled: { settings: boolean; edit: boolean }
  handleVariable: Function
  getVariable: Function
}) => {
  function variableSwitch() {
    switch (props.variable.type) {
      case "row":
        return (
          <Row
            id={props.variable.id}
            value={props.variable.value}
            disabled={props.disabled?.edit}
            handleVariable={props.handleVariable}
          />
        )
      case "table":
        return (
          <Table
            id={props.variable.id}
            value={props.variable.value}
            disabled={props.disabled?.edit}
            handleVariable={props.handleVariable}
          />
        )
      case "function":
        return (
          <Function
            id={props.variable.id}
            value={props.variable.value}
            disabled={props.disabled?.edit}
            handleVariable={props.handleVariable}
          />
        )
      default:
        return <p>Error: Unknown Variable Type</p>
    }
  }

  return (
    <Box>
      {props.variable && variableSwitch()}{" "}
      {!props.disabled?.settings && (
        <Button
          color="primary"
          outlined={true}
          size="small"
          fullwidth={true}
          title="Settings"
          onClick={(event: any) => {
            event.preventDefault()
            console.log("TODO edit settings")
          }}
        >
          <Icon className="primary">
            <FontAwesomeIcon icon={faCog} size="1x" />
          </Icon>
          <span>Settings</span>
        </Button>
      )}
    </Box>
  )
}

export default Variable
