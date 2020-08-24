import React, { useState } from "react"
import { Button, Icon } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faPlus } from "@fortawesome/free-solid-svg-icons"

import OptionsBox from "./OptionsBox"

const NewButton = () => {
  const [show, setShow] = useState(false)

  if (show) {
    return <OptionsBox />
  }

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
        setShow(true)
      }}
    >
      <Icon size="small" className="primary">
        <FontAwesomeIcon icon={faPlus} size="1x" />
      </Icon>
      <span>New</span>
    </Button>
  )
}

export default NewButton
