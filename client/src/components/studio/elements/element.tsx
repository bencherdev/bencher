import React from "react"
import { Box, Icon, Button } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faPlus } from "@fortawesome/free-solid-svg-icons"

import Parent from "./parent"
import Input from "./input"
import Decision from "./decision"
import Function from "./function"
import Subflow from "./subflow"
import Output from "./output"

const Element = (props: {
  element: any
  handleElement: Function
  getElement: Function
  context: { parent: string; current: string }
  getSubflow: Function
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
            getSubflow={props.getSubflow}
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
              getSubflow={props.getSubflow}
            />
            {getAddButton()}
          </React.Fragment>
        )
      case "decision":
        return (
          <React.Fragment>
            <Decision
              id={props.element.id}
              value={props.element.value}
              handleElement={props.handleElement}
              getElement={props.getElement}
              context={props.context}
            />
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
            />
            {getAddButton()}
          </React.Fragment>
        )
      case "subflow":
        return (
          <React.Fragment>
            <Subflow
              id={props.element.id}
              value={props.element.value}
              handleElement={props.handleElement}
              getElement={props.getElement}
              context={props.context}
              getSubflow={props.getSubflow}
            />
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
            getSubflow={props.getSubflow}
          />
        )
      default:
        return <h4>Error: Unknown Element Type</h4>
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
