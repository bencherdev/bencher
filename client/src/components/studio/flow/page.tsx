import React from "react"
import { Columns } from "react-bulma-components"

import Element from "../elements/element"

const Page = (props: {
  subflow: any
  handleElement: Function
  getSubflowName: Function
}) => {
  function getElement(id: string): any {
    return props?.subflow?.elements?.[id]
  }

  return (
    <Columns centered={true}>
      <Columns.Column size="three-quarters">
        {props?.subflow?.order?.map((elementId: any) => {
          let element = props.subflow.elements?.[elementId]
          return (
            <div key={elementId}>
              <Element
                element={element}
                handleElement={props.handleElement}
                getElement={getElement}
                context={{
                  parent: props.subflow.parent,
                  current: props.subflow.id,
                }}
                getSubflowName={props.getSubflowName}
              />
              <br />
            </div>
          )
        })}
      </Columns.Column>
    </Columns>
  )
}

export default Page
