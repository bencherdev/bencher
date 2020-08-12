import React from "react"
import renderer from "react-test-renderer"
import Flexbox from "./index"

// Fork of https://github.com/zoopoetics/react-svg-flexbox
// TODO move over to React Hooks

/* eslint-disable max-statements */
describe("Flexbox", () => {
  const onLayout = jest.fn()

  let component
  let instance
  let tree

  beforeEach(() => {
    component = renderer.create(
      <Flexbox onLayout={onLayout}>
        <rect fill={"#f0c"} height={10} width={10} />
        <rect fill={"#ccc"} height={456} width={123} />
      </Flexbox>
    )
    instance = component.getInstance()
    tree = component.toJSON()
  })

  it("matches snapshot", () => {
    expect(tree).toMatchSnapshot()
  })

  describe("implements componentDidMount and", () => {
    it("forces an initial update", () => {
      instance.forceUpdate = jest.fn()
      instance.componentDidMount()
      expect(instance.forceUpdate).toHaveBeenCalled()
    })
  })

  describe("implements componentDidUpdate and", () => {
    it("flips the shouldUpdateAgain switch", () => {
      instance.componentDidUpdate()
      expect(instance.shouldUpdateAgain).toStrictEqual(false)
    })
    it("performs the expected sequence of actions if shouldUpdateAgain is true", () => {
      instance.getChildrenMeasured = jest.fn()
      instance.getFlattenedChildren = jest.fn()
      instance.getChildrenAsMergedStyles = jest.fn()
      instance.getComputedLayout = jest.fn()
      instance.setState = jest.fn()

      instance.componentDidUpdate()

      expect(instance.getChildrenMeasured).toHaveBeenCalled()
      expect(instance.getFlattenedChildren).toHaveBeenCalled()
      expect(instance.getChildrenAsMergedStyles).toHaveBeenCalled()
      expect(instance.getComputedLayout).toHaveBeenCalled()
      expect(instance.props.onLayout).toHaveBeenCalled()
      expect(instance.setState).toHaveBeenCalled()
    })
  })

  describe("has a getChildren function that", () => {
    it("returns all children flattened into a single array", () => {
      class ReactComponent extends React.Component {
        render() {
          return <g />
        }
      }

      // Null indexes and string literals will be ignored
      const children = [
        new ReactComponent(),
        {},
        null,
        "string literal",
        [{}, {}],
      ]

      expect(instance.getFlattenedChildren(children)).toHaveLength(4)
    })
  })

  describe("has a getChildrenAsMergedStyles function that", () => {
    it("returns an array of children with measurements merged into their styles", () => {
      const length = 10
      const fill = "#f0c"
      const props = {
        style: { fill },
      }
      const child = { props }
      const children = Array.from({ length }, () => child)
      const height = 80
      const width = 100
      const childMeasured = {
        height,
        width,
      }
      const childrenMeasured = Array.from({ length }, () => childMeasured)
      const childrenWithMergedStyles = instance.getChildrenAsMergedStyles(
        children,
        childrenMeasured
      )
      childrenWithMergedStyles.forEach(childWithMergedStyles => {
        const { style } = childWithMergedStyles
        expect(style.fill).toStrictEqual(fill)
        expect(style.height).toStrictEqual(height)
        expect(style.width).toStrictEqual(width)
      })
    })
  })

  describe("has a getChildrenMeasured function that", () => {
    it("returns an array of child measurements", () => {
      const height = 80
      const width = 100
      const childRef = {
        getBBox: () => ({
          height,
          width,
        }),
      }
      const length = 10
      const childRefs = Array.from({ length }, () => childRef)
      const childrenMeasured = instance.getChildrenMeasured(childRefs)
      expect(childrenMeasured).toHaveLength(length)
      childrenMeasured.forEach(childMeasured => {
        expect(childMeasured.height).toStrictEqual(height)
        expect(childMeasured.width).toStrictEqual(width)
      })
    })
  })

  describe("has a getComputedLayout function that", () => {
    it("returns a layout from the css-layout lib", () => {
      const childrenWithMergedStyles = []
      const style = {}
      const computedLayout = instance.getComputedLayout(
        childrenWithMergedStyles,
        style
      )
      /*
      We're not going to check the entire layout,
      because that's effectively double-testing
      that css-layout works. We know that css-layout
      is already well tested, so we can be confident
      that the expected attributes exist.
      */
      expect(computedLayout.children).toStrictEqual([])
      expect(computedLayout.lastLayout).toBeDefined()
      expect(computedLayout.layout).toBeDefined()
      expect(computedLayout.shouldUpdate).toBeDefined()
    })
  })

  describe("has a getLayoutAttributesForChild function that", () => {
    const left = 20
    const top = 40
    const layout = { left, top }
    const layoutChild = { layout }

    it("returns cx and cy for a circle element, offset by radius", () => {
      const type = "circle"
      const radius = 50
      const props = { r: radius }
      const child = { props, type }
      const layoutProps = instance.getLayoutAttributesForChild(
        child,
        layoutChild
      )
      expect(layoutProps.cx).toStrictEqual(left + radius)
      expect(layoutProps.cy).toStrictEqual(top + radius)
    })
    it("returns cx and cy for an ellipse element, offset by radii", () => {
      const type = "ellipse"
      const radiusX = 50
      const radiusY = 30
      const props = {
        rx: radiusX,
        ry: radiusY,
      }
      const child = { props, type }
      const layoutProps = instance.getLayoutAttributesForChild(
        child,
        layoutChild
      )
      expect(layoutProps.cx).toStrictEqual(left + radiusX)
      expect(layoutProps.cy).toStrictEqual(top + radiusY)
    })
    it("returns a translate transform for a group element", () => {
      const type = "g"
      const child = { type }
      const layoutProps = instance.getLayoutAttributesForChild(
        child,
        layoutChild
      )
      expect(layoutProps.transform).toStrictEqual("translate(20 40)")
    })
    it("returns a translate transform for a path element", () => {
      const type = "path"
      const child = { type }
      const layoutProps = instance.getLayoutAttributesForChild(
        child,
        layoutChild
      )
      expect(layoutProps.transform).toStrictEqual("translate(20 40)")
    })
    it("returns a translate transform for a polygon element", () => {
      const type = "polygon"
      const child = { type }
      const layoutProps = instance.getLayoutAttributesForChild(
        child,
        layoutChild
      )
      expect(layoutProps.transform).toStrictEqual("translate(20 40)")
    })
    it("returns a translate transform for a polyline element", () => {
      const type = "polyline"
      const child = { type }
      const layoutProps = instance.getLayoutAttributesForChild(
        child,
        layoutChild
      )
      expect(layoutProps.transform).toStrictEqual("translate(20 40)")
    })
    it("returns x and y for all other element types", () => {
      const type = "stegosaurus"
      const child = { type }
      const layoutProps = instance.getLayoutAttributesForChild(
        child,
        layoutChild
      )
      expect(layoutProps.x).toStrictEqual(left)
      expect(layoutProps.y).toStrictEqual(top)
    })
    it("returns an empty object if layoutChild is falsy", () => {
      const layoutProps = instance.getLayoutAttributesForChild({})
      expect(layoutProps).toStrictEqual({})
    })
  })
})
/* eslint-enable max-statements */
