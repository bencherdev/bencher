import React from "react"

import { Content } from "react-bulma-components"

import { faBars } from "@fortawesome/free-solid-svg-icons"

import PanelCard from "./PanelCard"

import getWorkflow from "../../utils/getWorkflow"

const Workflows = (props: { path: string; workflows: any }) => {
  return (
    <React.Fragment>
      {props?.workflows?.map((id: any, index: number) => {
        const workflow = getWorkflow(id)
        return (
          <PanelCard
            key={index}
            icon={faBars}
            to={`/studio/workflow/#${workflow?.id?.toLowerCase()}`}
            title={workflow?.name}
          >
            <Content>
              <p>{workflow?.description}</p>
            </Content>
          </PanelCard>
        )
      })}
    </React.Fragment>
  )
}

export default Workflows
