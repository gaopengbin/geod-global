import importlib.util,unittest
from pathlib import Path
spec=importlib.util.spec_from_file_location('campaign_report',Path(__file__).with_name('campaign-report.py'))
report=importlib.util.module_from_spec(spec);spec.loader.exec_module(report)

class CampaignReportTests(unittest.TestCase):
    def test_excludes_internal_and_development_and_does_not_join_browser_to_install(self):
        def event(identity,name,source='zhihu',kind='new_installation',environment='production'):
            return {'installationId':identity,'name':name,'platform':'browser' if kind=='website' else 'desktop','environment':environment,
                    'fields':{'acquisitionSource':source,'acquisitionCampaign':report.CAMPAIGN,'acquisitionKind':kind}}
        rows=[event('qa','artifact_validated'),event('dev','app_first_run',environment='development'),
              event('browser','tutorial_viewed',kind='website'),event('browser','tutorial_download_clicked',kind='website'),
              event('fresh','app_first_run'),event('fresh','search_completed'),event('fresh','download_completed'),
              event('fresh','artifact_validated'),event('fresh','artifact_opened'),
              event('old','search_completed',source='bilibili',kind='existing_installation')]
        result=report.summarize(rows,{'qa'});self.assertEqual(result['internalEventsExcluded'],1)
        z=result['channels']['zhihu'];self.assertEqual(z['downloadClicks'],1);self.assertEqual(z['observedDesktopInstallations'],1)
        self.assertEqual(z['newInstallationFirstRuns'],1);self.assertEqual(z['validatedFiles'],1)
        self.assertEqual(z['installationsOpeningResults'],1)
        b=result['channels']['bilibili'];self.assertEqual(b['newInstallationFirstRuns'],0);self.assertEqual(b['existingInstallations'],1)
        self.assertNotIn('conversionRate',z)
if __name__=='__main__':unittest.main()
