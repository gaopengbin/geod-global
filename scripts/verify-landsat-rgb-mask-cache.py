"""Shared offline persistence acceptance; Landsat outputs retain UInt16/NoData 0."""
import importlib.util
from pathlib import Path
spec=importlib.util.spec_from_file_location("mask_cache",Path(__file__).with_name("verify-modis-rgb-mask-cache.py"))
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
if __name__=="__main__":module.main()
