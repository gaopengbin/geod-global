// Reproduce the directory's release scope from the same records as the picker.
// A connector status is not a claim that every collection or tile was tested.
import {writeFileSync,readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {SOURCE_DIRECTORY,SOURCE_GROUPS} from '../prototype/src/source-directory.js';
import {catalogProducts} from '../prototype/src/product-catalog.js';

const scope={
  schemaVersion:'geod-source-release-scope/v1',
  version:JSON.parse(readFileSync(new URL('../package.json',import.meta.url),'utf8')).version,
  directoryEntries:SOURCE_DIRECTORY.length,
  pickerProducts:catalogProducts().length,
  groups:SOURCE_GROUPS.length,
  statuses:SOURCE_DIRECTORY.reduce((counts,source)=>({...counts,[source.status]:(counts[source.status]||0)+1}),{}),
  statusMeaning:'Connector availability only. Provider accounts, product coverage, file limits and per-workflow acceptance remain separate. Planned families with a partial action are not complete.',
  entries:SOURCE_DIRECTORY.map(({id,name,group,status,action})=>({id,name,group,status,action:action||null})),
};
const text=JSON.stringify(scope,null,2)+'\n';
const target=new URL('../docs/releases/source-scope.json',import.meta.url);
if(process.argv.includes('--check')) {
  if(readFileSync(target,'utf8')!==text)throw new Error('Release source scope differs from the current directory. Regenerate it before packaging.');
} else writeFileSync(target,text);
console.log(JSON.stringify({file:fileURLToPath(target),version:scope.version,entries:scope.directoryEntries,picker:scope.pickerProducts,groups:scope.groups,...scope.statuses}));
