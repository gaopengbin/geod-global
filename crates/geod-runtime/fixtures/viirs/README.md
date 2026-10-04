These are **synthetic HDF5 fixtures**, not NASA observations or evidence of
authenticated production access. h5py 3.15.1 independently generates three
full 1200 × 1200 Int16 grids per product: M5/red, M4/green and M3/blue. The
metadata follows the reviewed 09A1 v002 parameters in the
[NASA user guide](https://lpdaac.usgs.gov/documents/1657/VNP09_User_Guide_V2.pdf).

The fixtures exercise little/big endian, HDF5 superblocks from earliest/latest
format, contiguous storage, irregular chunks, gzip, shuffle and Fletcher32.
NoData and an out-of-range DN are intentional. `expected.json` records the
SHA-256 of every original DN encoded little-endian, counts and extrema computed
with NumPy. Rust tests compare all decoded pixels through these hashes.

Regenerate from the repository root using an isolated QA Python environment:

```
python -m pip install h5py==3.15.1
python -X utf8 scripts/generate-viirs-fixture.py
```

Python/h5py and a native HDF5 installation are not desktop runtime dependencies.
The actual v002 provider schema and account download still need a real product
acceptance pass; these fixtures cannot establish that acceptance.
