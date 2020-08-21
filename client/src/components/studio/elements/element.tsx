import React from "react"

import Parent from "./parent"
import Input from "./input"
import Row from "./row"
import Decision from "./decision"
import Function from "./function"
import Subflow from "./subflow"
import Chart from "./chart"
import Output from "./output"

const Element = (props: {
  element: any
  handleElement: Function
  handleVariable: Function
  getVariable: Function
  context: { parent: string; current: string }
  getSubflow: Function
}) => {
  function elementSwitch() {
    switch (props.element.type) {
      case "parent":
        return <Parent context={props.context} getSubflow={props.getSubflow} />
      case "input":
        return (
          <Input
            id={props.element.id}
            value={props.element.value}
            handleElement={props.handleElement}
            handleVariable={props.handleVariable}
            getVariable={props.getVariable}
            context={props.context}
            getSubflow={props.getSubflow}
          />
        )
      case "row":
        return (
          <Row
            id={props.element.id}
            value={props.element.value}
            handleElement={props.handleElement}
            handleVariable={props.handleVariable}
            getVariable={props.getVariable}
          />
        )
      case "decision":
        return (
          <Decision
            id={props.element.id}
            value={props.element.value}
            handleElement={props.handleElement}
            handleVariable={props.handleVariable}
            getVariable={props.getVariable}
          />
        )
      case "function":
        return (
          <Function
            id={props.element.id}
            value={props.element.value}
            handleElement={props.handleElement}
            handleVariable={props.handleVariable}
            getVariable={props.getVariable}
          />
        )
      case "subflow":
        return (
          <Subflow
            id={props.element.id}
            value={props.element.value}
            handleElement={props.handleElement}
            handleVariable={props.handleVariable}
            getVariable={props.getVariable}
            getSubflow={props.getSubflow}
          />
        )
      case "chart":
        return (
          <Chart
            id={props.element.id}
            value={props.element.value}
            handleElement={props.handleElement}
            handleVariable={props.handleVariable}
            getVariable={props.getVariable}
          />
        )
      case "output":
        return (
          <Output
            id={props.element.id}
            value={props.element.value}
            handleElement={props.handleElement}
            handleVariable={props.handleVariable}
            getVariable={props.getVariable}
            context={props.context}
            getSubflow={props.getSubflow}
          />
        )
      default:
        return <h4>Error: Unknown Element Type</h4>
    }
  }

  return <React.Fragment>{props.element && elementSwitch()}</React.Fragment>
}

export default Element
