#!/usr/bin/env python3
"""Verify VAT fixtures against python-stdnum without adding a runtime dependency."""

from __future__ import annotations

import json
import pathlib
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
FIXTURE_PATH = ROOT / "tests" / "fixtures" / "stdnum-vat-cases.json"


def main() -> int:
    try:
        from stdnum.eu import vat
    except ImportError:
        print(
            "python-stdnum is required for this dev check. Install with: "
            "python3 -m pip install python-stdnum",
            file=sys.stderr,
        )
        return 2

    fixtures = json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))
    errors: list[str] = []

    for case in fixtures["validVatIds"]:
        if not vat.is_valid(case["value"]):
            errors.append(f"expected valid VAT ID: {case['country']} {case['value']}")

    for case in fixtures["invalidVatIds"]:
        if vat.is_valid(case["value"]):
            errors.append(f"expected invalid VAT ID: {case['country']} {case['value']}")

    for value in fixtures["validDutchBtwTaxNumbers"]:
        if vat.is_valid(value):
            errors.append(
                f"bare Dutch BTW tax number should not be a VIES-style VAT ID: {value}"
            )

    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1

    print(f"Verified {FIXTURE_PATH.relative_to(ROOT)} with python-stdnum.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
