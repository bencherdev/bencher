import React from "react"
import { Section, Columns, Box, Icon, Button } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faArrowUp } from "@fortawesome/free-solid-svg-icons"

import Element from "../elements/element"

const Page = (props: {
  subflow: any
  handleElement: Function
  getSubflow: Function
}) => {
  function getElement(id: string): any {
    return props?.subflow?.elements?.[id]
  }

  return (
    <Columns centered={true} gapless={true}>
      <Columns.Column narrow={true} size="three-quarters">
        {props?.subflow?.order?.map((elementId: any) => {
          let element = props.subflow.elements?.[elementId]
          return (
            <div key={elementId}>
              <Element
                element={element}
                handleElement={props.handleElement}
                getElement={getElement}
                context={{
                  parent: props.subflow.parent,
                  current: props.subflow.id,
                }}
                getSubflow={props.getSubflow}
              />
              <br />
            </div>
          )
        })}
        <br />
        <Box>
          <Button
            color="primary"
            outlined={true}
            fullwidth={true}
            onClick={(event: any) => {
              event.preventDefault()
              window.scrollTo(0, 0)
            }}
          >
            <Icon className="primary">
              <FontAwesomeIcon icon={faArrowUp} size="1x" />
            </Icon>
            <span>Back to Top</span>
          </Button>
        </Box>
        <Section>
          <br />
        </Section>
      </Columns.Column>
    </Columns>
  )
}

export default Page
