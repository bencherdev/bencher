import React from "react"
import { navigate } from "gatsby"
import { Container, Columns, Button, Icon } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faChevronLeft } from "@fortawesome/free-solid-svg-icons"

const Back = (props: { id: string }) => {
  return (
    <Container>
      <Columns className="is-paddingless">
        <Columns.Column className="is-marginless">
          {/*
                This uses an invisible character at the end
                https://www.compart.com/en/unicode/U+2800
            */}
          <small> ⠀⠀</small>
          <Button
            color="primary"
            size="small"
            inverted={true}
            state="loading"
            title="Back to Studio"
            onClick={(event: any) => {
              event.preventDefault()
              navigate(`/studio/flow/#${props?.id?.toLowerCase()}`)
            }}
          >
            <FontAwesomeIcon icon={faChevronLeft} size="1x" />
            <span> ⠀Back to Edit</span>
          </Button>
        </Columns.Column>
      </Columns>
    </Container>
  )
}

export default Back
