"""Verify the reviewed test fixture matches static assessment candidates."""

import json
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SCRIPT = ROOT / "skills/fastapi-assessment/scripts/analyze.py"
APP = ROOT / "examples/migration/fastapi_service.py"
CONTRACT = ROOT / "compiler/tests/generated_project/handoff_contract.json"


class ContractHandoffTest(unittest.TestCase):
    def test_reviewed_fixture_uses_only_assessed_candidates(self):
        result = subprocess.run(
            [sys.executable, str(SCRIPT), str(APP)],
            check=True,
            capture_output=True,
            text=True,
        )
        assessment = json.loads(result.stdout)
        endpoint = assessment["endpoints"][0]
        self.assertEqual(endpoint["path"], "/v1/users/{user_id}")
        self.assertEqual(endpoint["authentication"]["status"], "review_required")
        self.assertEqual(endpoint["side_effects"], "unknown")
        self.assertEqual(endpoint["shadow_execution"], "blocked_until_review")
        self.assertIsNone(endpoint["contract_candidate"]["operation_id"])
        self.assertTrue(endpoint["blockers"])
        contract = json.loads(CONTRACT.read_text(encoding="utf-8"))
        self.assertEqual(contract["schema_version"], endpoint["contract_candidate"]["schema_version"])
        self.assertEqual(contract["methods"][0]["name"], endpoint["contract_candidate"]["method_name"])
        self.assertEqual(contract["methods"][0]["operation_id"], 1)  # Test-only assignment.
        for actual, key in zip(contract["messages"], ("request_fields", "response_fields")):
            expected = [
                {"name": field["name"], "type": field["type"]["candidate"]}
                for field in endpoint[key]
            ]
            self.assertEqual(actual["fields"], expected)


if __name__ == "__main__":
    unittest.main()
