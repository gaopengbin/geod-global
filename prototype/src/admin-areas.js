const fold = value => String(value || '').normalize('NFKD').replace(/[\u0300-\u036f]/g, '').toLocaleLowerCase();

export function administrativePlace(feature, kind) {
  const properties = feature.getProperties();
  const country = kind === 'country';
  return {
    kind,
    feature,
    code: country ? properties.ADM0_A3 : properties.adm1_code,
    parentCode: country ? '' : properties.adm0_a3,
    nameEn: country ? (properties.NAME_EN || properties.NAME) : (properties.name_en || properties.name),
    nameZh: country ? properties.NAME_ZH : properties.name_zh,
    bounds: feature.getGeometry().getExtent(),
  };
}

export function searchAdministrativePlaces(places, query, limit = 8) {
  const term = fold(query.trim());
  if (!term) return [];
  return places
    .map(place => ({ place, names: [place.nameEn, place.nameZh, place.code, place.parentCode].map(fold) }))
    .filter(({ names }) => names.some(name => name.includes(term)))
    .sort((a, b) => {
      const rank = ({ place, names }) => (names.some(name => name === term) ? 0 : names.some(name => name.startsWith(term)) ? 1 : 2) + (place.kind === 'country' ? 0 : 0.1);
      return rank(a) - rank(b) || a.place.nameEn.localeCompare(b.place.nameEn);
    })
    .slice(0, limit)
    .map(({ place }) => place);
}
