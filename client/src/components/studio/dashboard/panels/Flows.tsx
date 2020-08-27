import React from "react"

import { Content } from "react-bulma-components"

import { faCircle } from "@fortawesome/free-regular-svg-icons"

import PanelCard from "./PanelCard"

import getFlow from "../../utils/getFlow"

const Flows = (props: { path: string; flows: any }) => {
  return (
    <React.Fragment>
      {props?.flows?.map((id: any, index: number) => {
        const flow = getFlow(id)
        return (
          <PanelCard
            key={index}
            icon={faCircle}
            to={`/studio/flow/#${flow?.id?.toLowerCase()}`}
            title={flow?.name}
          >
            <Content>
              <p>{flow?.description}</p>
            </Content>
          </PanelCard>
        )
      })}
    </React.Fragment>
  )
}

export default Flows
