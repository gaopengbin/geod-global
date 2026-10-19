// Actual isolated native API metadata, for contract/UI tests only.
// Original-file transfer and inspection acceptance remain separate evidence.
import fixture from './public-dataset-deafrica.fixture.json' with { type: 'json' };
export const deafricaDatasets = fixture.datasets;
export const deafricaCases = fixture.cases;
