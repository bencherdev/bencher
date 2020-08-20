import React from "react"
import { Box, Button, Icon, Level, Heading } from "react-bulma-components"

import Variable from "./variables/variable"
import Signature from "./variables/signature"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faTable, faArrowRight, faCog } from "@fortawesome/free-solid-svg-icons"

const Argument = (props: {
  variable: any
  disabled: { settings: boolean; edit: boolean }
  handleVariable: Function
  getVariable: Function
}) => {
  // TODO actually check things once Elements and Varibles are broken out
  function argumentSwitch(variable: string) {
    switch (variable) {
      case "row":
      // faLock
      case "table":
      // faTable
      case "function":
      // faEquals
      default:
        return (
          <React.Fragment>
            <Level>
              <Level.Side align="left">
                <Level.Item>
                  {/* TODO change color to red if there is an input error */}
                  <span className="icon is-medium has-text-primary">
                    <FontAwesomeIcon
                      // TODO make this a more advanced switch statement
                      icon={faTable}
                      size="2x"
                    />
                  </span>
                </Level.Item>
              </Level.Side>
              <br />
              <Level.Side align="center">
                <Level.Item>
                  <Heading size={5}>Table Signature</Heading>
                </Level.Item>
              </Level.Side>
              <br />
              <Level.Side align="right">
                <Level.Item>
                  <Button
                    color="primary"
                    outlined={true}
                    size="small"
                    fullwidth={true}
                    title="Settings"
                    onClick={(event: any) => {
                      console.log("TODO Argument Settings")
                    }}
                  >
                    <span className="icon">
                      <FontAwesomeIcon icon={faCog} size="1x" />
                    </span>
                    <span>Settings</span>
                  </Button>
                </Level.Item>
              </Level.Side>
            </Level>

            <Signature
              id={""}
              value={{}}
              disabled={true}
              handleVariable={() => {}}
            />
            <Button
              color="primary"
              size="small"
              fullwidth={true}
              title="Select input"
              onClick={(event: any) => {
                event.preventDefault()
                console.log("TODO select intput for argument")
              }}
            >
              <Icon className="primary">
                <FontAwesomeIcon icon={faArrowRight} size="1x" />
              </Icon>
              <span>Select Input</span>
            </Button>
          </React.Fragment>
        )
      // default:
      //   return <p>Error: Unknown Argument Type</p>
    }
  }

  return (
    <React.Fragment>
      {props.variable ? (
        <Variable
          variable={props.variable}
          disabled={props.disabled}
          handleVariable={props.handleVariable}
          getVariable={props.getVariable}
        />
      ) : (
        <Box>{argumentSwitch("TODO")}</Box>
      )}
    </React.Fragment>
  )
}

export default Argument
