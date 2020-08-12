/**
 * Implement Gatsby's Node APIs in this file.
 *
 * See: https://www.gatsbyjs.org/docs/node-apis/
 */
const path = require("path")
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin")

exports.onCreateWebpackConfig = ({ actions }, options) => {
  actions.setWebpackConfig({
    module: {
      rules: [
        {
          test: /\.wasm$/,
          type: "javascript/auto",
          loaders: ["wasm-loader"],
        },
      ],
    },
    plugins: [
      new WasmPackPlugin({
        crateDirectory: path.resolve(__dirname, "../interpreter"),
        outDir: "wasm",
      }),
    ],
  })
}
