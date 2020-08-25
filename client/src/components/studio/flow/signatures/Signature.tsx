import React, { ReactNode } from "react"
import { Button, Icon, Level, Heading } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import {
  faTimes,
  faCog,
  faArrowRight,
  IconDefinition,
} from "@fortawesome/free-solid-svg-icons"

const Signature = (props: {
  children: ReactNode
  name: string
  icon: IconDefinition
  handleArgument: Function
}) => {
  return (
    <React.Fragment>
      <Level>
        <Level.Side align="left">
          <Level.Item>
            <Icon size="medium" className="has-text-primary">
              <FontAwesomeIcon
                icon={props.icon ? props.icon : faTimes}
                size="2x"
              />
            </Icon>
          </Level.Item>
        </Level.Side>
        <br />
        <Level.Side align="center">
          <Level.Item>
            <Heading size={5}>
              {props.name ? props.name : "Unknown Signature"}
            </Heading>
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
                event.preventDefault()
                props?.handleArgument()
              }}
            >
              <Icon>
                <FontAwesomeIcon icon={faCog} size="1x" />
              </Icon>
              <span>Settings</span>
            </Button>
          </Level.Item>
        </Level.Side>
      </Level>
      {props.children}
      <Button
        color="primary"
        size="small"
        fullwidth={true}
        title="Select input"
        onClick={(event: any) => {
          event.preventDefault()
          props?.handleArgument()
        }}
      >
        <Icon className="primary">
          <FontAwesomeIcon icon={faArrowRight} size="1x" />
        </Icon>
        <span>Select Input</span>
      </Button>
    </React.Fragment>
  )
}

export default Signature
