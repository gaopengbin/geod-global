import importlib.util
from pathlib import Path
spec=importlib.util.spec_from_file_location('coupled_mcp',Path(__file__).with_name('verify-modis-rgb-mask-mcp.py'))
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
if __name__=='__main__':module.main()
