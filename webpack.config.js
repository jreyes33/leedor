const path = require("path")
const dist = path.resolve(__dirname, "dist")
const crate = path.resolve(__dirname, "crate")
const CopyPlugin = require("copy-webpack-plugin")
const HtmlWebpackPlugin = require("html-webpack-plugin")
const CspHtmlWebpackPlugin = require("csp-html-webpack-plugin")
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
    new CopyPlugin([{from: "static", to: "static"}]),
    new HtmlWebpackPlugin({
      template: "index.html",
      favicon: "favicon.ico",
    }),
    new CspHtmlWebpackPlugin({
      "default-src": "'self'",
      "base-uri": "'none'",
      "object-src": "'none'",
      "img-src": ["'self'", "data:"],
      "style-src": ["'self'", "'unsafe-inline'", "data:"],
      "script-src": ["'strict-dynamic'", "'unsafe-inline'", "'unsafe-eval'", "https:"],
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
