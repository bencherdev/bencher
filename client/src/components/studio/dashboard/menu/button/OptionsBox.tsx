import React from "react"
import { navigate } from "gatsby"
import { Box, Button, Icon, Columns } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faBorderNone } from "@fortawesome/free-solid-svg-icons"
import { faCircle, IconDefinition } from "@fortawesome/free-regular-svg-icons"

const OptionButton = (props: { name: string; icon: IconDefinition }) => (
  <Button
    color="primary"
    outlined={true}
    size="small"
    fullwidth={true}
    rounded={true}
    title={`New ${props.name}`}
    onClick={(event: any) => {
      event.preventDefault()
      navigate(`/studio/${props.name.toLowerCase()}s/new`)
    }}
  >
    <Icon size="small" className="primary">
      <FontAwesomeIcon icon={props.icon} size="1x" />
    </Icon>
    <span>New {props.name}</span>
  </Button>
)

const OptionsBox = () => {
  return (
    <React.Fragment>
      <Columns>
        <Columns.Column>
          <OptionButton name="Flow" icon={faCircle} />
        </Columns.Column>
        <Columns.Column>
          <OptionButton name="Template" icon={faBorderNone} />
        </Columns.Column>
      </Columns>
    </React.Fragment>
  )
}

export default OptionsBox
