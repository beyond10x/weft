import bundle from 'virtual:ess-ui/operator';
import { boot } from './boot.tsx';
import { operatorHosts } from './hosts/index.tsx';

boot(bundle, operatorHosts, { name: 'Operator console', other: { label: 'Account console', url: '../account/' } });
