import React from "react"

import { Content } from "react-bulma-components"

import PanelCard from "./PanelCard"

import getTemplate from "../../utils/getTemplate"
import { faBorderNone } from "@fortawesome/free-solid-svg-icons"

const Templates = (props: { templates: any }) => {
  return (
    <React.Fragment>
      {props?.templates?.map((id: any, index: number) => {
        const template = getTemplate(id)
        return (
          <PanelCard
            key={index}
            icon={faBorderNone}
            to={`/studio/templates/#${template?.id?.toLowerCase()}`}
            title={template?.name}
          >
            <Content>
              <p>{template?.description}</p>
            </Content>
          </PanelCard>
        )
      })}
    </React.Fragment>
  )
}

export default Templates
