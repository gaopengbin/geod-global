const fold = value => String(value || '').normalize('NFKD').replace(/[\u0300-\u036f]/g, '').toLocaleLowerCase();

// Natural Earth grouping codes locate source geometry. They do not determine
// the political/administrative category displayed by the application.
const regionPresentation = {
  HKG: { nameZh: '香港', levelLabel: 'Special administrative region', displayParentCode: 'CHN', aliases: ['香港特别行政区', '香港特別行政區', 'Hong Kong SAR'] },
  MAC: { nameZh: '澳门', levelLabel: 'Special administrative region', displayParentCode: 'CHN', aliases: ['澳門', '澳门特别行政区', '澳門特別行政區', 'Macao', 'Macau SAR'] },
  TWN: { nameZh: '台湾', levelLabel: 'Region', aliases: ['台灣', '臺灣', '台湾地区', '台灣地區', '臺灣地區', 'Taiwan'] },
};

function presentPlace(place, extraAliases = []) {
  const wholeRegionCode = place.kind === 'country' ? place.code : place.code === 'MAC+00?' ? 'MAC' : '';
  const presentation = regionPresentation[wholeRegionCode] || {};
  return { ...place, levelLabel: place.kind === 'country' ? 'Country / region' : 'Administrative area',
    ...presentation, canonicalRegionCode: wholeRegionCode,
    aliases: [place.nameZh, ...extraAliases, ...(presentation.aliases || [])].filter(Boolean) };
}

export function administrativePlace(feature, kind, source = 'Natural Earth 1:50m') {
  const properties = feature.getProperties();
  const country = kind === 'country';
  return presentPlace({
    kind,
    feature,
    code: country ? properties.ADM0_A3 : properties.adm1_code,
    parentCode: country ? '' : properties.adm0_a3,
    nameEn: country ? (properties.NAME_EN || properties.NAME) : (properties.name_en || properties.name),
    nameZh: country ? properties.NAME_ZH : properties.name_zh,
    nameLocal: country ? '' : properties.name_local,
    bounds: feature.getGeometry().getExtent(),
    source,
    clipLimitation: properties.clip_limitation || '',
  }, [country ? properties.NAME_ZHT : properties.name_zht]);
}

export function indexedAdministrativePlace(entry) {
  if (!entry || typeof entry.code !== 'string' || !/^[A-Z]{3}$/.test(entry.parentCode)
    || !Array.isArray(entry.bounds) || entry.bounds.length !== 4 || !entry.bounds.every(Number.isFinite))
    throw new Error('Invalid bundled administrative-area index');
  return presentPlace({ kind: 'province', feature: null, code: entry.code, parentCode: entry.parentCode,
    nameEn: entry.nameEn || entry.code, nameZh: entry.nameZh || '', nameLocal: entry.nameLocal || '',
    bounds: entry.bounds, source: 'Natural Earth 1:10m', clipLimitation: entry.clipLimitation || '' });
}

export function searchAdministrativePlaces(places, query, limit = 8) {
  const term = fold(query.trim());
  if (!term) return [];
  const topLevelCodes = new Set(places.filter(place => place.kind === 'country').map(place => place.code));
  return places
    .filter(place => place.kind === 'country' || !place.canonicalRegionCode || !topLevelCodes.has(place.canonicalRegionCode))
    .map(place => ({ place, names: [place.nameEn, place.nameZh, place.nameLocal, place.code, place.parentCode, ...(place.aliases || [])].map(fold) }))
    .filter(({ names }) => names.some(name => name.includes(term)))
    .sort((a, b) => {
      const rank = ({ place, names }) => (names.some(name => name === term) ? 0 : names.some(name => name.startsWith(term)) ? 1 : 2) + (place.kind === 'country' ? 0 : 0.1);
      return rank(a) - rank(b) || a.place.nameEn.localeCompare(b.place.nameEn);
    })
    .slice(0, limit)
    .map(({ place }) => place);
}
