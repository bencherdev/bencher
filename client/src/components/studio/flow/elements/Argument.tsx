import React from "react"

import Variable from "../variables/Variable"
import TableSignature from "../signatures/TableSignature"

const Argument = (props: {
  parameter:
    | undefined
    | {
        id: string
        type: string
        value: any
      }
  handleArgument: Function
  variable: any
  disabled: { settings: boolean; edit: boolean }
  handleVariable: Function
  getVariable: Function
}) => {
  // TODO actually check things once Elements and Varibles are broken out
  function signatureSwitch() {
    switch (props?.parameter?.type) {
      case "row":
        // faLock
        return <></>
      case "table":
        return (
          <TableSignature
            value={props.parameter?.value}
            handleArgument={props.handleArgument}
          />
        )
      case "function":
        // faEquals
        return <></>
      case "chart":
        // faChartBar
        return <></>
      default:
        return <p>Error: Unknown Signature Type</p>
    }
  }

  if (props.variable && props.variable !== "") {
    return (
      <Variable
        variable={props.variable}
        disabled={props.disabled}
        handleVariable={props.handleVariable}
        getVariable={props.getVariable}
      />
    )
  }

  return <React.Fragment>{signatureSwitch()}</React.Fragment>
}

export default Argument
