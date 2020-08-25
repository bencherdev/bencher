import React from "react"

import {
  faCircle,
  faArrowRight,
  faLock,
  faQuestion,
  faEquals,
  faChartBar,
  faArrowLeft,
} from "@fortawesome/free-solid-svg-icons"

import ElementCard from "./ElementCard"
import Parent from "./Parent"
import Input from "./Input"
import Row from "./Row"
import Decision from "./Decision"
import Function from "./Function"
import Subflow from "./Subflow"
import Chart from "./Chart"
import Output from "./Output"

import getFlow from "../../utils/getFlow"

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
        return (
          <ElementCard
            icon={faCircle}
            name={props?.getSubflow(props.context?.parent)?.name}
          >
            <Parent />
          </ElementCard>
        )
      case "input":
        return (
          <ElementCard
            icon={faArrowRight}
            name={`Input to ${
              props?.getSubflow(props.context?.current)?.name
            } Subflow`}
          >
            <Input
              id={props.element.id}
              value={props.element.value}
              handleElement={props.handleElement}
              handleVariable={props.handleVariable}
              getVariable={props.getVariable}
              context={props.context}
            />
          </ElementCard>
        )
      case "row":
        const row = props.getVariable(props.element.value?.id)
        return (
          <ElementCard icon={faLock} name={row?.value?.name}>
            <Row
              id={props.element.id}
              row={row}
              handleElement={props.handleElement}
              handleVariable={props.handleVariable}
              getVariable={props.getVariable}
            />
          </ElementCard>
        )
      case "decision":
        const decisionValue = props.element.value
        return (
          <ElementCard icon={faQuestion} name={decisionValue?.name}>
            <Decision
              id={props.element.id}
              value={decisionValue}
              handleElement={props.handleElement}
              handleVariable={props.handleVariable}
              getVariable={props.getVariable}
            />
          </ElementCard>
        )
      case "function":
        const functionValue = props.element.value
        const flow = getFlow(functionValue?.id)
        return (
          <ElementCard icon={faEquals} name={flow?.name}>
            <Function
              id={props.element.id}
              value={functionValue}
              flow={flow}
              handleElement={props.handleElement}
              handleVariable={props.handleVariable}
              getVariable={props.getVariable}
            />
          </ElementCard>
        )
      case "subflow":
        const subflowValue = props.element.value
        const subflow = props.getSubflow(subflowValue?.id)
        return (
          <ElementCard icon={faCircle} name={subflow?.name}>
            <Subflow
              id={props.element.id}
              value={subflowValue}
              subflow={subflow}
              handleElement={props.handleElement}
              handleVariable={props.handleVariable}
              getVariable={props.getVariable}
              getSubflow={props.getSubflow}
            />
          </ElementCard>
        )
      case "chart":
        const chartValue = props.element.value
        const chartTable = props.getVariable(chartValue?.id)
        const chartConfig = chartValue?.config
        return (
          <ElementCard icon={faChartBar} name={chartConfig?.name}>
            <Chart
              id={props.element.id}
              config={chartConfig}
              table={chartTable}
              handleElement={props.handleElement}
              handleVariable={props.handleVariable}
              getVariable={props.getVariable}
            />
          </ElementCard>
        )
      case "output":
        return (
          <ElementCard
            icon={faArrowLeft}
            name={`Output from ${
              props?.getSubflow(props.context?.current)?.name
            } Subflow`}
          >
            <Output
              id={props.element.id}
              value={props.element.value}
              handleElement={props.handleElement}
              handleVariable={props.handleVariable}
              getVariable={props.getVariable}
              context={props.context}
            />
          </ElementCard>
        )
      default:
        return <h4>Error: Unknown Element Type</h4>
    }
  }

  return <React.Fragment>{props.element && elementSwitch()}</React.Fragment>
}

export default Element
