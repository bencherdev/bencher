import React from "react"
import { render } from "react-dom"
import { range as d3range } from "d3"
import { Circle, Rectangle, Triangle } from "./icons"
import AccordionFlow from "./AccordionFlow"
import { Content, DynamicContent } from "./Content"

const styles = {
  fontFamily: "sans-serif",
  textAlign: "center",
}

const icons = [<Circle />, <Rectangle />, <Triangle />],
  flowData = d3range(10).map(i => [
    icons[i % 3],
    contentUpdated => (
      <DynamicContent
        key={i}
        title={`Row ${i}`}
        contentUpdated={contentUpdated}
      >
        {d3range(10)
          .slice(0, i)
          .map(
            () => `Once upon a midnight dreary, while I pondered, weak and weary,\n
            Over many a quaint and curious volume of forgotten lore—\n
            While I nodded, nearly napping, suddenly there came a tapping,\n
            As of some one gently rapping, rapping at my chamber door.\n
            “’Tis some visitor,” I muttered, “tapping at my chamber door—\n
            Only this and nothing more.”`
          )}
      </DynamicContent>
    ),
  ])

const Alpha = () => <AccordionFlow data={flowData} />

export default Alpha
