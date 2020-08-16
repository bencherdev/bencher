import React from "react"
import { Columns } from "react-bulma-components"

import Element from "../elements/element"

const Page = (props: { subflow: any; handleElement: Function }) => {
  return (
    <Columns centered={true}>
      <Columns.Column size="three-quarters">
        {props?.subflow &&
          props?.subflow?.order.map((elementId: any) => {
            let element = props?.subflow?.elements?.[elementId]
            return (
              <Element
                key={elementId}
                element={element}
                handleElement={props.handleElement}
              />
            )
          })}
      </Columns.Column>
    </Columns>
  )
}

export default Page
