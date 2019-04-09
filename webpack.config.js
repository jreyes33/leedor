const path = require("path")
const dist = path.resolve(__dirname, "dist")
const crate = path.resolve(__dirname, "crate")
const HtmlWebpackPlugin = require("html-webpack-plugin")
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin")
const webpack = require("webpack")

module.exports = {
  entry: "./js/index.js",
  output: {
    path: dist,
    filename: "bundle.js",
  },
  devServer: {
    contentBase: dist,
    hot: true,
  },
  plugins: [
    new HtmlWebpackPlugin({
      template: "index.html",
      favicon: "favicon.ico",
    }),
    new WasmPackPlugin({
      crateDirectory: crate,
    }),
    new webpack.HotModuleReplacementPlugin(),
  ],
  module: {
    rules: [
      {
        test: /\.css$/,
        use: ["style-loader", "css-loader"],
      },
    ],
  },
}
