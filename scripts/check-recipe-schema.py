"""Keep published executable recipe examples inside their versioned contracts.

This validates file structure, not local-source availability or spatial results.
Runtime tests and independent raster QA cover those separate requirements.
"""
import copy
import json
from pathlib import Path
from jsonschema import Draft202012Validator, FormatChecker

root = Path(__file__).resolve().parents[1]
schema = json.loads((root / "schemas/raster-recipe-v1.schema.json").read_text(encoding="utf-8"))
Draft202012Validator.check_schema(schema)
validator = Draft202012Validator(schema, format_checker=FormatChecker())
example = json.loads((root / "examples/sentinel-scl-clip.recipe.json").read_text(encoding="utf-8"))
validator.validate(example)
invalid = []
for field, value in [("schemaVersion", "future/v2"), ("command", "external program"), ("name", "\n")]:
    fixture = copy.deepcopy(example)
    fixture[field] = value
    invalid.append(fixture)
for field, value in [("type", "shell"), ("crs", "EPSG:3857"), ("bounds", [1, 2, 3]), ("bounds", [0, 85, 1, 86])]:
    fixture = copy.deepcopy(example)
    fixture["operation"][field] = value
    invalid.append(fixture)
fixture = copy.deepcopy(example)
fixture["source"]["path"] = "arbitrary-local-path.tif"
invalid.append(fixture)
fixture = copy.deepcopy(example)
fixture["output"]["format"] = "COG"
invalid.append(fixture)
for fixture in invalid:
    assert not validator.is_valid(fixture), "Unsupported recipe accepted by the published schema"
polygon_schema = json.loads((root / "schemas/raster-recipe-v2.schema.json").read_text(encoding="utf-8"))
Draft202012Validator.check_schema(polygon_schema)
polygon_validator = Draft202012Validator(polygon_schema, format_checker=FormatChecker())
polygon_example = json.loads((root / "examples/sentinel-scl-polygon-clip.recipe.json").read_text(encoding="utf-8"))
polygon_validator.validate(polygon_example)
assert not validator.is_valid(polygon_example), "v1 accepted polygon recipe"
assert not polygon_validator.is_valid(example), "v2 accepted rectangular recipe"
for change in [
    {"geometry": {"type": "LineString", "coordinates": []}},
    {"crs": "source"},
    {"path": "arbitrary.tif"},
]:
    fixture = copy.deepcopy(polygon_example)
    fixture["operation"].update(change)
    assert not polygon_validator.is_valid(fixture), "Invalid polygon recipe accepted"
print(json.dumps({"executableRecipeSchema": "passed", "validExamples": 2, "rejectedExamples": len(invalid) + 5, "scope": "structure only; runtime preflight remains required"}))
