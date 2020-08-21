import React from "react"
import { Link, navigate } from "gatsby"

import { Columns, Content, Button, Breadcrumb } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faChevronLeft } from "@fortawesome/free-solid-svg-icons"

const FlowBreadcrumb = (props: any) => (
  <Columns className="is-vcentered">
    <Columns.Column size={1}>
      <Content className="has-text-centered">
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
      </Content>
    </Columns.Column>
    <Columns.Column>
      <Breadcrumb
        renderAs={Link}
        hrefAttr="to"
        items={[
          {
            name: "Main Subflow",
            url: "#a/1",
          },
          {
            name: "Nested Subflow",
            url: "#a/2",
          },
          {
            name: "Lower Subflow",
            url: "#a/3",
            active: true,
          },
        ]}
      />
    </Columns.Column>
  </Columns>
)

export default FlowBreadcrumb
