// Actual isolated native resolve/connect/inspection responses, frozen for
// consumer contract and UI tests. Mock playback is not live download evidence.
import fixture from './public-dataset-next.fixture.json' with { type: 'json' };
export const nextPublicDatasets = fixture.datasets;
export const nextPublicCases = fixture.cases;
export const nextPublicReceipt = {
  source: fixture.receipt,
  checkedAt: fixture.checkedAt,
  binarySha256: fixture.binarySha256,
  status: fixture.receiptStatus,
};
