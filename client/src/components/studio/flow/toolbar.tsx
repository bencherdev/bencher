import React from "react"
import { Columns, Button } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import {
  faUndoAlt,
  faRedoAlt,
  faSearchMinus,
  faSearchPlus,
  faArrowCircleDown,
  faCloudUploadAlt,
} from "@fortawesome/free-solid-svg-icons"
import { faCircle } from "@fortawesome/free-regular-svg-icons"

const Toolbar = (props: any) => (
  <Columns breakpoint="mobile">
    <Columns.Column className="is-gapless is-narrow">
      <Button
        color="primary"
        size="medium"
        inverted={true}
        title="Back to Studio"
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
  </Columns>
)

export default Toolbar
