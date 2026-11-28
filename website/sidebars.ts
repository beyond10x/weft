import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docsSidebar: [
    'index',
    'getting-started',
    {
      type: 'category',
      label: 'Concepts',
      collapsed: false,
      link: {
        type: 'generated-index',
        slug: '/concepts',
        description: 'The objects weft manages, the components that will run it and the services it will offer by name.',
      },
      items: [
        'concepts/tenants-and-accounts',
        'concepts/formations',
        'concepts/workloads',
        'concepts/claims',
        'concepts/nodes-and-clusters',
        'concepts/components',
        'concepts/managed-services',
      ],
    },
    {
      type: 'category',
      label: 'The specification',
      collapsed: false,
      items: ['specification/read', 'specification/layers', 'specification/validate'],
    },
    {
      type: 'category',
      label: 'cloudspec-sim',
      collapsed: false,
      items: ['simulator/install', 'simulator/run', 'simulator/results'],
    },
    'run/consoles',
    {
      type: 'category',
      label: 'Run weft (Planned)',
      collapsed: false,
      items: ['run/control-host-and-node', 'run/deploy-a-workload'],
    },
    {
      type: 'category',
      label: 'Operate',
      collapsed: false,
      items: ['operate/troubleshoot'],
    },
    'status',
  ],
};

export default sidebars;
