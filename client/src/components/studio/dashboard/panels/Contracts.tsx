import React from "react"

import { Content } from "react-bulma-components"

import { faEllipsisH } from "@fortawesome/free-solid-svg-icons"

import PanelCard from "./PanelCard"

import getContract from "../../utils/getContract"

const Contracts = (props: { path: string; contracts: any }) => {
  return (
    <React.Fragment>
      {props?.contracts?.map((id: any, index: number) => {
        const contract = getContract(id)
        return (
          <PanelCard
            key={index}
            icon={faEllipsisH}
            to={`/studio/contract/#${contract?.id?.toLowerCase()}`}
            title={contract?.name}
          >
            <Content>
              <p>{contract?.description}</p>
            </Content>
          </PanelCard>
        )
      })}
    </React.Fragment>
  )
}

export default Contracts
