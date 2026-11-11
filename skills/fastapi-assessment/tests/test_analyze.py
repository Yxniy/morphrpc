import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "analyze.py"
spec = importlib.util.spec_from_file_location("fastapi_assessment", SCRIPT)
analyzer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(analyzer)


class AssessmentTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def write(self, name, source):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(source, encoding="utf-8")
        return path

    def test_aliases_router_prefix_models_and_dependency_signals(self):
        self.write(
            "app.py",
            """
from fastapi import FastAPI as App, APIRouter as Router, Depends as D
from pydantic import BaseModel
app = App(dependencies=[D(global_auth)])
router = Router(prefix="/users")
class User(BaseModel):
    name: str
    enabled: bool
    age: int
@router.get("/{user_id}", response_model=User)
async def get_user(user_id: int, token: str = D(auth)):
    return User()
app.include_router(router, prefix="/v1")
""",
        )
        report = analyzer.analyze(self.root)
        self.assertEqual(report["assessment_version"], 1)
        self.assertEqual(report["migration_plan"]["status"], "analysis_only")
        self.assertEqual(len(report["endpoints"]), 1)
        route = report["endpoints"][0]
        self.assertEqual(
            (route["method"], route["path"]), ("GET", "/v1/users/{user_id}")
        )
        self.assertTrue(route["async"])
        self.assertEqual(route["request_fields"][0]["source"], "path")
        self.assertIsNone(route["request_fields"][0]["type"]["candidate"])
        self.assertEqual(
            [item["type"]["candidate"] for item in route["response_fields"]],
            ["string", "bool", None],
        )
        self.assertIn("D", route["authentication"]["dependency_signals"])
        self.assertIn(
            "declared_dependencies", route["authentication"]["dependency_signals"]
        )
        self.assertEqual(route["side_effects"], "unknown")
        self.assertIsNone(route["contract_candidate"]["operation_id"])
        self.assertIsNone(route["contract_candidate"]["service"])
        self.assertEqual(route["contract_candidate"]["schema_version"], 1)
        self.assertTrue(route["blockers"])

    def test_unsupported_routes_dynamic_mounts_and_syntax_errors(self):
        self.write(
            "app.py",
            """
from fastapi import FastAPI, APIRouter
app = FastAPI()
router = APIRouter()
@router.get("/item")
def item(x: str): return x
app.include_router(imported_router, prefix=PREFIX)
app.add_api_route("/dynamic", item)
@app.api_route("/multi", methods=["GET", "POST"])
def multi(): pass
""",
        )
        self.write("broken.py", "def broken(:\n")
        report = analyzer.analyze(self.root)
        self.assertIsNone(report["endpoints"][0]["path"])
        self.assertTrue(
            any("Router mount" in text for text in report["endpoints"][0]["blockers"])
        )
        self.assertEqual(
            [d["code"] for d in report["diagnostics"]],
            [
                "dynamic_registration",
                "dynamic_registration",
                "unsupported_decorator",
                "parse_error",
            ],
        )

    def test_nested_mounts_remain_unresolved(self):
        self.write(
            "app.py",
            """
from fastapi import FastAPI, APIRouter
app = FastAPI()
outer = APIRouter(prefix="/outer")
inner = APIRouter(prefix="/inner")
@inner.get("/route")
def route(): pass
outer.include_router(inner, prefix="/nested")
app.include_router(outer, prefix="/v1")
""",
        )
        report = analyzer.analyze(self.root)
        self.assertIsNone(report["endpoints"][0]["path"])
        self.assertTrue(
            any(d["code"] == "dynamic_registration" for d in report["diagnostics"])
        )

    def test_compiler_identifiers_and_rust_keywords_require_review(self):
        self.write(
            "app.py",
            """
from fastapi import FastAPI
from pydantic import BaseModel
app = FastAPI()
class Payload(BaseModel):
    valid_field: str
    userId: str
    gen: str
    café: str
@app.get("/", response_model=Payload)
def new(): pass
""",
        )
        endpoint = analyzer.analyze(self.root)["endpoints"][0]
        fields = {entry["name"]: entry for entry in endpoint["response_fields"]}
        self.assertEqual(fields["valid_field"]["blockers"], [])
        for name in ["userId", "gen", "café"]:
            self.assertTrue(fields[name]["blockers"], name)
        self.assertTrue(any("Method name" in item for item in endpoint["blockers"]))

    def test_router_inclusion_before_route_does_not_claim_registration(self):
        self.write(
            "app.py",
            """
from fastapi import FastAPI, APIRouter
app = FastAPI()
router = APIRouter()
app.include_router(router, prefix="/v1")
@router.get("/late")
def late(): pass
""",
        )
        report = analyzer.analyze(self.root)
        self.assertIsNone(report["endpoints"][0]["path"])
        self.assertTrue(
            any(d["code"] == "mount_before_route" for d in report["diagnostics"])
        )

    def test_duplicate_routes_flag_precedence_for_review(self):
        self.write(
            "app.py",
            """
from fastapi import FastAPI
app = FastAPI()
@app.get("/same")
def first(): pass
@app.get("/same")
def second(): pass
""",
        )
        endpoints = analyzer.analyze(self.root)["endpoints"]
        self.assertEqual(len(endpoints), 2)
        for endpoint in endpoints:
            self.assertTrue(any("precedence" in item for item in endpoint["blockers"]))

    def test_no_execution_or_source_mutation_and_deterministic_cli(self):
        marker = self.root / "executed.txt"
        app = self.write(
            "app.py",
            """
from pathlib import Path
Path(__file__).with_name("executed.txt").write_text("executed")
from fastapi import FastAPI
app = FastAPI()
@app.get("/")
def index(value: str): return value
""",
        )
        before = app.read_bytes()
        first = subprocess.run(
            [sys.executable, str(SCRIPT), str(self.root)],
            check=True,
            capture_output=True,
            text=True,
        )
        second = subprocess.run(
            [sys.executable, str(SCRIPT), str(self.root)],
            check=True,
            capture_output=True,
            text=True,
        )
        self.assertEqual(first.stdout, second.stdout)
        self.assertEqual(app.read_bytes(), before)
        self.assertFalse(marker.exists())
        self.assertEqual(json.loads(first.stdout)["endpoints"][0]["path"], "/")

    def test_unresolved_types_defaults_and_expanded_options_are_blocked(self):
        self.write(
            "app.py",
            """
from fastapi import FastAPI
from typing import Optional
app = FastAPI()
@app.post("/", **ROUTE_OPTIONS)
def create(value: Optional[str] = None) -> dict: return {}
""",
        )
        endpoint = analyzer.analyze(self.root)["endpoints"][0]
        self.assertFalse(endpoint["request_fields"][0]["required"])
        self.assertIsNone(endpoint["request_fields"][0]["type"]["candidate"])
        self.assertTrue(
            any("Expanded route options" in blocker for blocker in endpoint["blockers"])
        )
        self.assertTrue(
            any("Response model" in blocker for blocker in endpoint["blockers"])
        )

    def test_skips_links_and_environments_and_reports_oversized_source(self):
        self.write(".venv/hidden.py", "raise RuntimeError()")
        self.write("large.py", "#" * (analyzer.MAX_SOURCE_BYTES + 1))
        self.write("source.py", "x = 1\n")
        (self.root / "linked.py").symlink_to(self.root / "source.py")
        report = analyzer.analyze(self.root)
        self.assertEqual(report["scope"]["python_files_discovered"], 2)
        self.assertEqual(
            {item["code"] for item in report["diagnostics"]},
            {"file_too_large", "skipped_symlink"},
        )
        self.assertEqual(report["scope"]["coverage"], "partial")

    def test_invalid_input_fails_without_traceback(self):
        result = subprocess.run(
            [sys.executable, str(SCRIPT), str(self.root / "missing")],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("assessment failed", result.stderr)
        self.assertNotIn("Traceback", result.stderr)


if __name__ == "__main__":
    unittest.main()
