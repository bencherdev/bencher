import React from "react"
import { Button, Content, Heading, Box, Level } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faTable, faTimes, faPen } from "@fortawesome/free-solid-svg-icons"

const Argument = (props: { element: any; disabled: boolean }) => {
  return (
    <Box>
      <Level breakpoint="mobile">
        <Level.Side align="left">
          <Level.Item>
            {/* TODO change color to red if there is an input error */}
            <span className="icon is-large has-text-primary">
              <FontAwesomeIcon
                // TODO make this a more advanced switch statement
                icon={props.element?.type === "table" ? faTable : faTimes}
                size="3x"
              />
            </span>
          </Level.Item>
        </Level.Side>
        <br />
        <Level.Side align="center">
          <Level.Item>
            <Content className="has-text-centered">
              <Heading size={5}>
                {props.element?.value?.name?.toString()}
              </Heading>
            </Content>
          </Level.Item>
        </Level.Side>
        <br />
        <Level.Side align="right">
          {!props.disabled && (
            <Level.Item>
              <Button
                size="small"
                onClick={(event: any) => {
                  console.log("TODO Edit Argument")
                }}
              >
                <span className="icon">
                  <FontAwesomeIcon icon={faPen} size="1x" />
                </span>
                <span>Edit</span>
              </Button>
            </Level.Item>
          )}
        </Level.Side>
      </Level>
    </Box>
  )
}

export default Argument
