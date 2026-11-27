import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';
import {withProductSite} from '@beyond10x/docs-system/product-site';
import rehypeOptionAnchors from './src/rehype/option-anchors';
import rehypeTableLabels from './src/rehype/table-labels';
import rehypeCodeBreaks from './src/rehype/code-breaks';

const config: Config = {
  title: 'weft',
  tagline: 'A compute cloud you deploy yourself, specified first in ESS.',
  favicon: 'img/favicon.svg',

  future: {
    v4: true,
  },

  url: 'https://beyond10x.github.io',
  baseUrl: '/weft/',

  organizationName: 'beyond10x',
  projectName: 'weft',
  deploymentBranch: 'gh-pages',
  trailingSlash: false,

  onBrokenLinks: 'throw',
  onBrokenAnchors: 'throw',

  markdown: {
    format: 'detect',
    hooks: {
      onBrokenMarkdownLinks: 'throw',
    },
    mermaid: true,
  },

  clientModules: ['./src/client/search-row.ts', './src/client/search-focus.ts'],

  themes: [
    '@docusaurus/theme-mermaid',
    [
      '@easyops-cn/docusaurus-search-local',
      {
        hashed: true,
        language: ['en'],
        indexDocs: true,
        indexBlog: false,
        indexPages: false,
        docsRouteBasePath: 'docs',
        highlightSearchTermsOnTargetPage: true,
        explicitSearchResultPath: true,
      },
    ],
  ],

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          routeBasePath: 'docs',
          editUrl: 'https://github.com/beyond10x/weft/tree/main/website/',
          rehypePlugins: [rehypeOptionAnchors, rehypeTableLabels, rehypeCodeBreaks],
        },
        blog: false,
        theme: {customCss: './src/css/custom.css'},
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    navbar: {
      title: 'weft',
      logo: {alt: 'weft', src: 'img/logo.svg', width: 32, height: 32},
      items: [
        {to: '/docs/', label: 'Documentation', position: 'left'},
        {to: '/docs/getting-started', label: 'Getting started', position: 'left'},
        {to: '/docs/specification/read', label: 'Specification', position: 'left'},
        {to: '/docs/simulator/run', label: 'weft-sim', position: 'left'},
        {to: '/docs/status', label: 'Status', position: 'left'},
        {href: 'https://github.com/beyond10x/weft', label: 'GitHub', position: 'right'},
      ],
    },
    footer: {
      links: [
        {
          title: 'Documentation',
          items: [
            {label: 'What weft is', to: '/docs/'},
            {label: 'Getting started', to: '/docs/getting-started'},
            {label: 'Concepts', to: '/docs/concepts/tenants-and-accounts'},
            {label: 'Read the specification', to: '/docs/specification/read'},
            {label: 'Validate the specification', to: '/docs/specification/validate'},
            {label: 'weft-sim', to: '/docs/simulator/run'},
            {label: 'Status', to: '/docs/status'},
          ],
        },
        {
          title: 'Project',
          items: [
            {label: 'Source', href: 'https://github.com/beyond10x/weft'},
            {label: 'Domain sketch', href: 'https://github.com/beyond10x/weft/blob/main/docs/domain-sketch.md'},
            {label: 'ESS', href: 'https://beyond10x.github.io/ess/'},
            {label: 'Spindle', href: 'https://beyond10x.github.io/spindle/'},
          ],
        },
      ],
      copyright: 'A beyond10x project. weft.',
    },
  } satisfies Preset.ThemeConfig,
};

export default withProductSite(config, {landing: './product.json', mark: 'We'});
