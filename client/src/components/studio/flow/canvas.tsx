import React from "react"

import Element from "../modeler/element"

const Canvas = (props: {
  canvas: { width: string; height: string }
  flow: any
  subflow: any
  handleElement: Function
}) => {
  return (
    <svg width="100%" height="2000">
      {props?.flow?.subflows?.[props?.subflow]?.lines &&
        props?.flow?.subflows?.[props?.subflow]?.lines?.map(
          (line: any, lineIndex: number) => {
            // TODO break this into its own Line component
            // This component will keep state for the line
            // such as the midpoints, include when "wrap text" occurs
            return line?.map((elementId: any, positionIndex: number) => {
              let elements = props?.flow?.subflows?.[props?.subflow]?.elements
              return (
                <Element
                  key={lineIndex.toString() + ":" + positionIndex.toString()}
                  location={{ line: lineIndex, position: positionIndex }}
                  prior={
                    positionIndex === 0
                      ? null
                      : elements?.[line[positionIndex - 1]]
                  }
                  element={elements?.[elementId]}
                  handleElement={props.handleElement}
                />
              )
            })
          }
        )}
    </svg>
  )
}

export default Canvas
