import React from "react"
import { navigate } from "gatsby"
import { Columns, Button } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import {
  faUndoAlt,
  faRedoAlt,
  faSearchMinus,
  faSearchPlus,
  faArrowCircleDown,
  faCloudUploadAlt,
  faChevronLeft,
} from "@fortawesome/free-solid-svg-icons"
import { faCircle } from "@fortawesome/free-regular-svg-icons"

import Breadcrumb from "./Breadcrumb"

const Toolbar = (props: { flowId: string }) => (
  <Columns breakpoint="mobile" className="is-vcentered">
    <Columns.Column className="is-gapless is-narrow">
      <Button
        color="primary"
        size="medium"
        inverted={true}
        title="Back to Studio"
        onClick={(event: any) => {
          event.preventDefault()
          navigate("/studio")
        }}
      >
        <FontAwesomeIcon icon={faChevronLeft} size="1x" />
      </Button>
    </Columns.Column>
    <Columns.Column className="is-gapless is-narrow">
      <Button
        color="primary"
        size="medium"
        inverted={true}
        title="View Flow App"
        onClick={(event: any) => {
          event.preventDefault()
          navigate(`/flow/#${props.flowId}`)
        }}
      >
        <FontAwesomeIcon icon={faCircle} size="2x" />
      </Button>
    </Columns.Column>
    <Columns.Column className="is-gapless is-narrow">
      <Button color="primary" size="medium" inverted={true} title="Undo">
        <FontAwesomeIcon icon={faUndoAlt} size="1x" />
      </Button>
      <Button color="primary" size="medium" inverted={true} title="Redo">
        <FontAwesomeIcon icon={faRedoAlt} size="1x" />
      </Button>
    </Columns.Column>
    <Columns.Column className="is-gapless is-narrow">
      <Button color="primary" size="medium" inverted={true} title="Zoom Out">
        <FontAwesomeIcon icon={faSearchMinus} size="1x" />
      </Button>
      <Button color="primary" size="medium" inverted={true} title="Zoom In">
        <FontAwesomeIcon icon={faSearchPlus} size="1x" />
      </Button>
    </Columns.Column>
    <Columns.Column className="is-gapless is-narrow">
      <Button color="primary" size="medium" inverted={true} title="Download">
        <FontAwesomeIcon icon={faArrowCircleDown} size="1x" />
      </Button>
      <Button color="primary" size="medium" inverted={true} title="Cloud Sync">
        <FontAwesomeIcon icon={faCloudUploadAlt} size="1x" />
      </Button>
    </Columns.Column>
    <Columns.Column className="is-gapless is-narrow">
      <Breadcrumb />
    </Columns.Column>
  </Columns>
)

export default Toolbar
