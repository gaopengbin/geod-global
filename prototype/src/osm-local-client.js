const keys=['conversionVersion','encoding','schemaVersion','objectCounts','generator','source','datasetTimestamp','declaredBounds','requiredFeatures','optionalFeatures','replicationSequence','replicationBaseUrl','ignoredBlockTypes','xmlRootAttributes'];
const plain=v=>v!==null&&typeof v==='object'&&!Array.isArray(v);
export function validateLocalOsm(s,count) {
  const text=v=>v===null||typeof v==='string';const fail=()=>{throw new Error('Invalid local OSM original-file provenance.');};
  if(!plain(s)||Object.keys(s).length!==keys.length||keys.some(k=>!(k in s))||s.conversionVersion!==1||!['xml','pbf'].includes(s.encoding)||s.schemaVersion!=='0.6'
    ||!plain(s.objectCounts)||Object.keys(s.objectCounts).length!==3||['node','way','relation'].some(k=>!Number.isSafeInteger(s.objectCounts[k])||s.objectCounts[k]<0||s.objectCounts[k]>50000)||Object.values(s.objectCounts).reduce((a,b)=>a+b,0)!==count
    ||['generator','source','datasetTimestamp','replicationSequence','replicationBaseUrl'].some(k=>!text(s[k]))||s.datasetTimestamp!==null&&!Number.isFinite(Date.parse(s.datasetTimestamp))
    ||s.replicationSequence!==null&&!/^\d+$/.test(s.replicationSequence)
    ||s.declaredBounds!==null&&(!Array.isArray(s.declaredBounds)||s.declaredBounds.length!==4||!s.declaredBounds.every(Number.isFinite)||s.declaredBounds[0]<-180||s.declaredBounds[2]>180||s.declaredBounds[1]<-90||s.declaredBounds[3]>90||s.declaredBounds[0]>s.declaredBounds[2]||s.declaredBounds[1]>s.declaredBounds[3])
    ||['requiredFeatures','optionalFeatures','ignoredBlockTypes'].some(k=>!Array.isArray(s[k])||s[k].length>64||s[k].some(v=>typeof v!=='string'))
    ||s.requiredFeatures.some(f=>!['OsmSchema-V0.6','DenseNodes'].includes(f))||!plain(s.xmlRootAttributes)||Object.values(s.xmlRootAttributes).some(v=>typeof v!=='string')
    ||new TextEncoder().encode(JSON.stringify(s)).length>65536)fail();
  if(s.encoding==='pbf'&&(!s.requiredFeatures.includes('OsmSchema-V0.6')||Object.keys(s.xmlRootAttributes).length)||s.encoding==='xml'&&(s.requiredFeatures.length||s.optionalFeatures.length||s.ignoredBlockTypes.length||s.replicationSequence!==null||s.replicationBaseUrl!==null||s.source!==null))fail();
  return s;
}
export function validateLocalOsmFeatures(features,source) {
  const counts={node:0,way:0,relation:0},seen=new Set();
  for(const f of features){const p=f.properties,kind=p?.osm_type,id=p?.osm_id;
    if(!(kind in counts)||!Number.isSafeInteger(id)||id<=0||f.id!==`${kind}/${id}`||seen.has(f.id)||!plain(p.tags)||Object.values(p.tags).some(v=>typeof v!=='string'))throw new Error('Local OSM objects differ from their recorded identities or counts.');
    seen.add(f.id);counts[kind]++;
  }
  if(Object.entries(counts).some(([k,n])=>n!==source.objectCounts[k]))throw new Error('Local OSM objects differ from their recorded identities or counts.');
}
