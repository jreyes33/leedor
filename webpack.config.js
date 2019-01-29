const path = require("path")
const dist = path.resolve(__dirname, "dist")
const crate = path.resolve(__dirname, "crate")
const HtmlWebpackPlugin = require("html-webpack-plugin")
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin")

module.exports = {
  entry: "./js/index.js",
  output: {
    path: dist,
    filename: "bundle.js",
  },
  devServer: {
    contentBase: dist,
  },
  plugins: [
    new HtmlWebpackPlugin({
      template: "index.html",
    }),
    new WasmPackPlugin({
      crateDirectory: crate,
    }),
  ],
}
