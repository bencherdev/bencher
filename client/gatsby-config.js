module.exports = {
  siteMetadata: {
    title: `TableFlow - Build Modular Spreadsheets`,
    description: `Build modular business and financial modeling spreadsheets`,
    author: `@tableflow`,
  },
  plugins: [
    `gatsby-plugin-react-helmet`,
    `gatsby-plugin-remove-trailing-slashes`,
    {
      resolve: `gatsby-source-filesystem`,
      options: {
        name: `images`,
        path: `${__dirname}/src/images`,
      },
    },
    {
      resolve: `gatsby-plugin-manifest`,
      options: {
        name: `TabeFlow - Build Modular Spreadsheets`,
        short_name: `TableFlow`,
        start_url: `/about`,
        background_color: `#FFFFFF`,
        theme_color: `#4386FA`,
        display: `minimal-ui`,
        // This path is relative to the root of the site.
        icon: `./src/images/tableflow-icon-512.png`,
        cache_busting_mode: `none`,
      },
    },
    {
      resolve: `gatsby-plugin-sass`,
      options: {
        includePaths: [`./src/styles`],
      },
    },
    `gatsby-plugin-styled-components`,
    `gatsby-plugin-fontawesome-css`,
    {
      resolve: `gatsby-plugin-web-font-loader`,
      options: {
        google: {
          families: [`Abel`],
        },
      },
    },
    `gatsby-transformer-sharp`,
    `gatsby-plugin-sharp`,
    {
      resolve: `gatsby-plugin-layout`,
      options: {
        component: require.resolve(`./src/components/utils/layout.tsx`),
      },
    },
    {
      resolve: `gatsby-plugin-create-client-paths`,
      options: { prefixes: [] },
    },
    {
      resolve: `gatsby-plugin-workerize-loader`,
      // options: {
      //   preloads: [`interpreter`],
      // },
    },
    {
      resolve: `gatsby-plugin-offline`,
      options: {
        precachePages: [`/about`, `/studio/*`, `/auth/*`],
        workboxConfig: {
          // globPatterns: ["**tableflow-icon*"],
        },
      },
    },
  ],
}
