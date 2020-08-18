import React from "react"
import { Box, Icon, Button } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faPlus } from "@fortawesome/free-solid-svg-icons"

import Parent from "./parent"
import Input from "./input"
import Function from "./function"
import Output from "./return"

const Element = (props: {
  element: any
  handleElement: Function
  getElement: Function
  context: { parent: string; current: string }
  getSubflowName: Function
}) => {
  function elementSwitch() {
    switch (props.element.type) {
      case "parent":
        return (
          <Parent
            id={props.element.id}
            value={props.element.value}
            handleElement={props.handleElement}
            context={props.context}
            getSubflowName={props.getSubflowName}
          />
        )
      case "input":
        return (
          <React.Fragment>
            <Input
              id={props.element.id}
              value={props.element.value}
              handleElement={props.handleElement}
              getElement={props.getElement}
              context={props.context}
              getSubflowName={props.getSubflowName}
            />
            {getAddButton()}
          </React.Fragment>
        )
      case "formula":
        return (
          <React.Fragment>
            <p>Formula</p>
            {getAddButton()}
          </React.Fragment>
        )
      case "function":
        return (
          <React.Fragment>
            <Function
              id={props.element.id}
              value={props.element.value}
              handleElement={props.handleElement}
              getElement={props.getElement}
              context={props.context}
              getSubflowName={props.getSubflowName}
            />
            {getAddButton()}
          </React.Fragment>
        )
      case "decision":
        return (
          <React.Fragment>
            <p>Decision</p>
            {getAddButton()}
          </React.Fragment>
        )
      case "subflow":
        return (
          <React.Fragment>
            <p>Subflow</p>
            {getAddButton()}
          </React.Fragment>
        )
      case "output":
        return (
          <Output
            id={props.element.id}
            value={props.element.value}
            handleElement={props.handleElement}
            getElement={props.getElement}
            context={props.context}
            getSubflowName={props.getSubflowName}
          />
        )
      default:
        return <p>Error: Unknown Element Type</p>
    }
  }

  function getAddButton() {
    return (
      <div>
        <br />
        <Box>
          <Button
            color="primary"
            outlined={true}
            fullwidth={true}
            onClick={(event: any) => {
              event.preventDefault()
              console.log("TODO add a new element")
            }}
          >
            <Icon className="primary">
              <FontAwesomeIcon icon={faPlus} size="1x" />
            </Icon>
            <span>Add</span>
          </Button>
        </Box>
      </div>
    )
  }

  return <React.Fragment>{props.element && elementSwitch()}</React.Fragment>
}

export default Element
