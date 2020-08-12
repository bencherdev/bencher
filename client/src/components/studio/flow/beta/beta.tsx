import React from "react"
import Circle from "./circle"
import AccordionFlow from "./AccordionFlow"
import DynamicContent from "./Content"

const flowData = [...Array(5).keys()].map(i => [
  <Circle />,
  contentUpdated => (
    <DynamicContent key={i} title={`Row ${i}`} contentUpdated={contentUpdated}>
      {[...Array(5).keys()].map(i => "Paragraph #" + i)}
    </DynamicContent>
  ),
])

const Beta = () => <AccordionFlow data={flowData} />

export default Beta
