import bundle from 'virtual:ess-ui/account';
import { boot } from './boot.tsx';
import { accountHosts } from './hosts/index.tsx';

boot(bundle, accountHosts, { name: 'Account console', other: { label: 'Operator console', url: '../operator/' } });
