/**
 * Implement Gatsby's Node APIs in this file.
 *
 * See: https://www.gatsbyjs.org/docs/node-apis/
 */

// You can delete this file if you're not using it
exports.onCreateWebpackConfig = ({ actions }, options) => {
  actions.setWebpackConfig({
    // module: {
    //   exprContextCritical: false,
    // },
    // module: {
    //   rules: [
    //     {
    //       test: /\.wasm$/,
    //       type: "webassembly/experimental",
    //       //   type: "javascript/auto",
    //       //   loaders: ["wasm-loader"],
    //     },
    //   ],
    // },
    // optimization: {
    //   chunkIds: "deterministic", // To keep filename consistent between different modes (for example building only)
    // },
    // experiments: {
    //   asyncWebAssembly: true,
    // },
    // module: {
    //   rules: [
    //     {
    //       test: /\.wasm$/,
    //       type: "webassembly/experimental",
    //     },
    //   ],
    // },
  })
}
