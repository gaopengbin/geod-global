import test from 'node:test';
import assert from 'node:assert/strict';
import fixture from './archive-connections-native.fixture.json' with {type:'json'};
import {validatePublicDatasetConnection,validatePublicDatasetItem} from './public-dataset-client.js';
import {reviewedArchiveSelection} from './original-backlog-client.js';
import {stacRequest} from './stac-client.js';

// Actual native /datasets/connect responses and immutable source snapshots.
// The fixture preserves the publisher ZIP MIME and member-specific proof.
const validateReceipt = receipt => {
  const {connection,request,snapshots}=receipt;
  validatePublicDatasetConnection(connection,request);
  for(const snapshot of snapshots) validatePublicDatasetItem(snapshot,connection.plan,connection.stacConnections);
};

for(const receipt of fixture.receipts) {
  test(`${receipt.request.datasetId}: actual original archive connection opens every complete reviewed member`,()=>{
    validateReceipt(receipt);
    for(const snapshot of receipt.snapshots) {
      const selected=reviewedArchiveSelection(snapshot.assets[0]);
      assert.ok(selected);
      assert.equal(snapshot.assets[0].mediaType,'application/zip');
      assert.notEqual(selected.archiveSha256,selected.memberSha256);
    }
  });
  test(`${receipt.request.datasetId}: original ZIP MIME, dual hashes, whole-member name and source proof cannot be replaced`,()=>{
    for(const mutate of [
      s=>s.assets[0].mediaType='image/tiff',
      s=>s.assets[0].mediaType='application/octet-stream',
      s=>s.assets[0].metadata.kind='direct-raster',
      s=>s.assets[0].metadata.reviewedArchive.archiveSha256='0'.repeat(64),
      s=>s.assets[0].metadata.reviewedArchive.memberSha256=s.assets[0].metadata.reviewedArchive.archiveSha256,
      s=>s.assets[0].metadata.reviewedArchive.member='other.tif',
      s=>s.assets[0].metadata.reviewedArchive.memberBytes++,
      s=>s.assets[0].metadata['file:checksum']='1220'+'0'.repeat(64),
      s=>s.properties.datasetPreset.sourceId='unreviewed-member',
      s=>s.assets[0].metadata.datasetPreset.request.bounds[0]+=0.001,
    ]) {
      const changed=structuredClone(receipt);mutate(changed.snapshots[0]);
      assert.throws(()=>validateReceipt(changed));
    }
    for(const mutate of [
      c=>c.stacConnections[0].metadataSha256[0]='0'.repeat(64),
      c=>c.stacConnections[0].snapshotIds[0]='0'.repeat(64),
      c=>c.stacConnections[0].name='a different source',
    ]) {
      const changed=structuredClone(receipt);mutate(changed.connection);
      assert.throws(()=>validateReceipt(changed));
    }
  });
}

const fabdem=fixture.receipts.find(r=>r.request.datasetId==='fabdem');
test('actual combined FABDEM receipts share only the original URL/connection, with two immutable proofs and selected TIFF members',()=>{
  const {connection,snapshots}=fabdem,[a,b]=connection.stacConnections;
  assert.equal(a.url,b.url);assert.equal(a.id,b.id);
  assert.notEqual(a.snapshotIds[0],b.snapshotIds[0]);
  assert.notEqual(a.metadataSha256[0],b.metadataSha256[0]);
  assert.notEqual(snapshots[0].assets[0].metadata.reviewedArchive.memberSha256,snapshots[1].assets[0].metadata.reviewedArchive.memberSha256);
  validateReceipt(fabdem);
});
test('combined archive receipts reject duplicate snapshots, replayed members, switched sources and rebound URL identities',()=>{
  for(const mutate of [
    r=>r.connection.stacConnections[1]=structuredClone(r.connection.stacConnections[0]),
    r=>r.connection.stacConnections.pop(),
    r=>r.connection.stacConnections[1].snapshotIds=[r.connection.stacConnections[0].snapshotIds[0]],
    r=>r.connection.stacConnections[1].snapshotIds.push(r.connection.stacConnections[0].snapshotIds[0]),
    r=>r.connection.stacConnections[1].id='00000000-0000-4000-8000-000000000001',
    r=>{const [a,b]=r.connection.stacConnections;[a.name,b.name]=[b.name,a.name];},
    r=>{const [a,b]=r.snapshots; b.assets[0].metadata.reviewedArchive=structuredClone(a.assets[0].metadata.reviewedArchive);},
    r=>{const [a,b]=r.snapshots; b.properties.datasetPreset=structuredClone(a.properties.datasetPreset);b.assets[0].metadata.datasetPreset=structuredClone(a.properties.datasetPreset);},
  ]) {
    const changed=structuredClone(fabdem);mutate(changed);
    assert.throws(()=>validateReceipt(changed));
  }
});
test('reading the N09 immutable snapshot rejects a replayed N08 member even though both use the same original ZIP and connection',async()=>{
  const previous=globalThis.fetch;
  globalThis.fetch=async()=>({ok:true,json:async()=>structuredClone(fabdem.snapshots[0])});
  try {
    await assert.rejects(stacRequest('snapshot',{id:fabdem.snapshots[1].id}),/requested snapshot/);
  } finally {globalThis.fetch=previous;}
});
