import React from 'react';
import {useI18n} from './i18n.jsx';
export function LocalOsmDetails({source}) {
  const {t,number}=useI18n();if(!source)return null;
  return <><dt>{t('Original format')}</dt><dd>OSM {source.encoding==='xml'?'XML':'PBF'} · {t('Original file retained byte for byte')}</dd>
    <dt>{t('OSM objects')}</dt><dd>{t('{nodes} nodes · {ways} ways · {relations} relations',{nodes:number(source.objectCounts.node),ways:number(source.objectCounts.way),relations:number(source.objectCounts.relation)})}</dd>
    <dt>{t('File generator')}</dt><dd>{source.generator||t('Not recorded in this file')}</dd>
    <dt>{t('OSM dataset timestamp')}</dt><dd>{source.datasetTimestamp||t('Not recorded in this file')}</dd>
    {source.declaredBounds&&<><dt>{t('Declared file extent')}</dt><dd>{source.declaredBounds.join(', ')}</dd></>}
    {source.replicationSequence!==null&&<><dt>{t('OSM replication sequence')}</dt><dd>{source.replicationSequence}</dd></>}
    {source.requiredFeatures.length>0&&<><dt>{t('OSM PBF required features')}</dt><dd>{source.requiredFeatures.join(', ')}</dd></>}
    {source.ignoredBlockTypes.length>0&&<><dt>{t('Unrendered extension blocks')}</dt><dd>{source.ignoredBlockTypes.join(', ')}</dd></>}
  </>;
}
