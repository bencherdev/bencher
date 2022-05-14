
import path from "path";
import { fileURLToPath } from "url";
const __dirname = path.dirname(fileURLToPath(import.meta.url));

import HtmlWebpackPlugin from "html-webpack-plugin";

export default {
  entry: './src/index.tsx',
  output: {
    filename: 'bundle.js',
    path: path.join(__dirname, 'dist')
  },
  devServer: {
    contentBase: path.join(__dirname, 'dist'),
    publicPath: '/'
  },
  mode: 'development',
  resolve: {
    extensions: ['.tsx', '.ts', '.js', '.json']
  },
  module: {
    rules: [
      {
        test: /\.(ts)x?$/,
        exclude: /node_modules/,
        use: {
          loader: 'babel-loader',
          options: {
            babelrc: false,
            configFile: false,
            presets: ['@babel/preset-env', 'solid', '@babel/preset-typescript'],
            plugins: ['@babel/plugin-syntax-dynamic-import', '@babel/plugin-proposal-class-properties', '@babel/plugin-proposal-object-rest-spread'],
          }
        }
      }
    ]
  },
  plugins: [
    new HtmlWebpackPlugin()
  ]
}
