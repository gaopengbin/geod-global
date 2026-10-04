// Product-specific scientific definitions; these are not generic image bands.
const layer = (asset, dataType, scale, nodata, validRange, kind, unit, catalogUnit, label) => Object.freeze({ asset, dataType, scale, nodata, validRange, kind, unit, catalogUnit, label });
export const MODIS_SCIENCE = Object.freeze({
  vi_quality: layer('250m_16_days_VI_Quality','uint16',1,65535,[0,65534],'flags','bit field',undefined,'VI quality flags'),
  vi_reliability: layer('250m_16_days_pixel_reliability','int8',1,-1,[0,3],'rank','rank','Rank','Pixel reliability'),
  vi_doy: layer('250m_16_days_composite_day_of_the_year','int16',1,-1,[1,366],'date','day of year','JulianDay','Pixel observation day'),
  vi_red: layer('250m_16_days_red_reflectance','int16',0.0001,-1000,[0,10000],'reflectance','reflectance',undefined,'VI red reflectance'),
  vi_nir: layer('250m_16_days_NIR_reflectance','int16',0.0001,-1000,[0,10000],'reflectance','reflectance',undefined,'VI near-infrared reflectance'),
  vi_blue: layer('250m_16_days_blue_reflectance','int16',0.0001,-1000,[0,10000],'reflectance','reflectance',undefined,'VI blue reflectance'),
  vi_mir: layer('250m_16_days_MIR_reflectance','int16',0.0001,-1000,[0,10000],'reflectance','reflectance',undefined,'VI mid-infrared reflectance'),
  vi_view_zenith: layer('250m_16_days_view_zenith_angle','int16',0.01,-10000,[0,18000],'angle','degrees','Degree','View zenith angle'),
  vi_sun_zenith: layer('250m_16_days_sun_zenith_angle','int16',0.01,-10000,[0,18000],'angle','degrees','Degree','Sun zenith angle'),
  vi_relative_azimuth: layer('250m_16_days_relative_azimuth_angle','int16',0.01,-4000,[-18000,18000],'angle','degrees','Degree','Relative azimuth angle'),
});
export const MODIS_SCIENCE_KEYS = Object.freeze(Object.keys(MODIS_SCIENCE));
export const MODIS_SCIENCE_DEFINITION = 'https://lpdaac.usgs.gov/documents/621/MOD13_User_Guide_V61.pdf';
export const MODIS_SCIENCE_PALETTE = 'modis13-science-v1';
export const MODIS_SCIENCE_COLORS = ['#2563eb','#d97706','#8b5cf6','#64748b'];
