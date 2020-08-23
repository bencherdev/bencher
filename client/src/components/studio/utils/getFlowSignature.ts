function getFlowSignature(flow: any): any {
  if (!flow) {
    return
  }

  let mainSubflowId = flow.main
  if (!mainSubflowId) {
    return
  }

  let mainSubflow = flow.subflows?.[mainSubflowId]
  if (!mainSubflow) {
    return
  }

  let inputElementId = mainSubflow.input
  let outputElementId = mainSubflow.output
  if (!inputElementId || !outputElementId) {
    return
  }

  let inputElement = mainSubflow.elements?.[inputElementId]
  let outputElement = mainSubflow.elements?.[outputElementId]
  if (!inputElement || !outputElement) {
    return
  }

  let mainSubflowInputs = inputElement.value?.inputs
  let mainSubflowOutputs = outputElement.value?.outputs
  if (!mainSubflowInputs || !mainSubflowOutputs) {
    return
  }

  return {
    id: flow.id,
    main: mainSubflowId,
    inputs: mainSubflowInputs,
    outputs: mainSubflowOutputs,
  }
}

export default getFlowSignature
