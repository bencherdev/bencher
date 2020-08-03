module.exports = {
  siteMetadata: {
    title: `TableFlow - Build Modular Spreadsheets`,
    description: `Build modular professional business and financial modeling spreadsheets`,
    author: `@tableflow`,
  },
  plugins: [
    `gatsby-plugin-react-helmet`,
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
        icon: `src/images/tableflow-icon-512.png`,
      },
    },
    {
      resolve: `gatsby-plugin-sass`,
      options: {
        includePaths: ["./src/styles"],
      },
    },
    {
      resolve: "gatsby-plugin-web-font-loader",
      options: {
        google: {
          families: ["Abel"],
        },
      },
    },
    {
      resolve: `gatsby-plugin-layout`,
      options: {
        component: require.resolve(`./src/components/utils/layout.tsx`),
      },
    },
    {
      resolve: `gatsby-plugin-create-client-paths`,
      options: { prefixes: [`/studio/*`, `/auth/*`] },
    },
    `gatsby-transformer-sharp`,
    `gatsby-plugin-sharp`,
    `gatsby-plugin-styled-components`,
    // To learn more, visit: https://gatsby.dev/offline
    // `gatsby-plugin-offline`,
  ],
}
