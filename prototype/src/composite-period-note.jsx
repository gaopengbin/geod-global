import React from 'react';import {InfoDetails} from './ui/index.jsx';import {useI18n} from './i18n.jsx';
export function CompositePeriodNote({scene}) {
 const {t}=useI18n(),declared=scene.properties?.end_datetime;
 if(!scene.endDate||!declared||Date.parse(declared)===Date.parse(scene.endDate))return null;
 return <InfoDetails summary={t('Composite period declaration')} closeLabel={t('Close')}><p>{t('The publisher catalog declares an interval ending {declared} UTC. The product-year composite is shown through {end} UTC. The original catalog declaration, source identity and downloaded values are retained.',{declared:declared.slice(0,10),end:scene.endDate.slice(0,10)})}</p></InfoDetails>;
}
