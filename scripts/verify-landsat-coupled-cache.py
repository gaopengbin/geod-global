import importlib.util
from pathlib import Path
spec=importlib.util.spec_from_file_location('coupled_cache',Path(__file__).with_name('verify-modis-rgb-mask-cache.py'))
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
if __name__=='__main__':module.main()
