// Selected actual native-tested cases only. Catalog coverage and live browser
// acceptance are separate; replay cannot stand in for a new original transfer.
import fixture from './public-dataset-next2.fixture.json' with { type: 'json' };

export const next2PublicDatasets = fixture.datasets;
export const next2PublicCases = fixture.cases;
export const next2PublicReceipts = fixture.receipts;
export const next2PublicFixtureScope = {
  label: fixture.label,
  fixtureScope: fixture.fixtureScope,
  playbackIsLiveBrowserAcceptance: fixture.playbackIsLiveBrowserAcceptance,
};
