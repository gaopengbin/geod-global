"""Export explicitly synthetic native evidence for UI contract regression tests."""
import argparse, json
from pathlib import Path
parser=argparse.ArgumentParser()
parser.add_argument('native_report',type=Path)
parser.add_argument('output',type=Path)
args=parser.parse_args()
report=json.loads(args.native_report.read_text(encoding='utf-8'))
assert report['provenance'].startswith('Synthetic HDF5 fixture outputs')
for job in [report['source'],*report['jobs'],report['clip']]:
    job.pop('outputPath',None)
    job.pop('manifestPath',None)
args.output.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print('Exported synthetic native UI fixture:',args.output)
