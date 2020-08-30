import React from "react"
import { navigate } from "gatsby"
import { Button, Icon, Columns } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faBars, faEllipsisH } from "@fortawesome/free-solid-svg-icons"
import {
  faCircle,
  faSquare,
  IconDefinition,
} from "@fortawesome/free-regular-svg-icons"

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
          <OptionButton name="Workflow" icon={faBars} />
        </Columns.Column>
        <Columns.Column>
          <OptionButton name="Flow" icon={faCircle} />
        </Columns.Column>
        <Columns.Column>
          <OptionButton name="Template" icon={faSquare} />
        </Columns.Column>
        <Columns.Column>
          <OptionButton name="Contract" icon={faEllipsisH} />
        </Columns.Column>
      </Columns>
    </React.Fragment>
  )
}

export default OptionsBox
