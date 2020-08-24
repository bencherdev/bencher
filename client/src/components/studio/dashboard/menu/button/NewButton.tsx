import React from "react"
import { navigate } from "gatsby"
import { Button, Icon } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faPlus } from "@fortawesome/free-solid-svg-icons"

const AddButton = (props: {}) => {
  return (
    <Button
      color="primary"
      outlined={true}
      size="medium"
      fullwidth={true}
      rounded={true}
      title="New"
      onClick={(event: any) => {
        event.preventDefault()
        // TODO in the future make this a dynamic button
        // It should show options for what to make a new instance of
        // Flow, Template, etc
        navigate("/studio/flows/new")
      }}
    >
      <Icon size="small" className="primary">
        <FontAwesomeIcon icon={faPlus} size="1x" />
      </Icon>
      <span>New</span>
    </Button>
  )
}

export default AddButton
